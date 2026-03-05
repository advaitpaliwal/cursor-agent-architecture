mod bench;
mod compositor;
mod config;
mod easing;
mod error;
mod logging;
mod plan;
mod proxy;
mod proxy_generation;
mod scheduler;
mod util;
mod video;

pub use bench::{
    bench_proxy_random_access, ProxyRandomAccessBenchConfig, ProxyRandomAccessBenchResult,
};
pub use compositor::effects::motion_blur::{MotionBlurConfig, MotionBlurQuality};
pub use config::*;
pub use error::{RendererError, Result};
pub use plan::types::*;
pub use proxy::{ProxyMode, SelectedInput};

use std::path::{Path, PathBuf};

use tracing::{info, warn};

use crate::plan::parser::{default_plan_path, load_plan, load_recording_data};
use crate::proxy::select_input;
use crate::proxy_generation::{ensure_render_proxies, ProxyGenerationRequest};
use crate::util::resolution::compute_target_dimensions;
use crate::video::ensure_ffmpeg_initialized;

pub struct RenderConfig {
    pub session_dir: PathBuf,
    pub plan_path: Option<PathBuf>,
    pub output_path: PathBuf,
    pub output_width: Option<u32>,
    pub proxy_mode: ProxyMode,
    pub realtime: bool,
    pub metrics_json: Option<PathBuf>,
    pub motion_blur: MotionBlurConfig,
}

pub fn run(config: RenderConfig) -> Result<()> {
    logging::init_logging();

    let plan_path = config
        .plan_path
        .clone()
        .unwrap_or_else(|| default_plan_path(&config.session_dir));
    let plan = load_plan(&plan_path)?;

    if !plan.diagnostics.errors.is_empty() {
        warn!("Plan diagnostics errors: {:?}", plan.diagnostics.errors);
    }
    if !plan.diagnostics.warnings.is_empty() {
        info!("Plan diagnostics warnings: {:?}", plan.diagnostics.warnings);
    }

    let recording_dir = config.session_dir.join("recording");
    let source_video_path = resolve_video_path(&plan.video.input_video_path, &config.session_dir);
    let mut recording_data = load_recording_data(&config.session_dir)?;
    let mut proxies = recording_data
        .as_ref()
        .and_then(|r| r.render_proxies.as_ref());

    let selected_input_result = select_input(
        config.proxy_mode,
        &recording_dir,
        &source_video_path,
        proxies,
        config.output_width,
    );

    let mut selected_input = match selected_input_result {
        Ok(input) => input,
        Err(err) => {
            if matches!(
                config.proxy_mode,
                ProxyMode::Auto | ProxyMode::Proxy1080p | ProxyMode::ProxyFull
            ) {
                let desired_width = config.output_width.unwrap_or(u32::MAX);
                let request = match config.proxy_mode {
                    ProxyMode::Proxy1080p => ProxyGenerationRequest {
                        generate_1080p: true,
                        generate_full: false,
                        full_required: false,
                    },
                    ProxyMode::ProxyFull => ProxyGenerationRequest {
                        generate_1080p: false,
                        generate_full: true,
                        full_required: true,
                    },
                    ProxyMode::Auto => ProxyGenerationRequest {
                        generate_1080p: true,
                        generate_full: desired_width > 1920,
                        full_required: false,
                    },
                    ProxyMode::None => ProxyGenerationRequest {
                        generate_1080p: false,
                        generate_full: false,
                        full_required: false,
                    },
                };

                warn!(
                    "Proxy selection failed ({}); attempting on-demand proxy generation",
                    err
                );
                ensure_render_proxies(
                    &config.session_dir,
                    &recording_dir,
                    &source_video_path,
                    request,
                )?;

                recording_data = load_recording_data(&config.session_dir)?;
                proxies = recording_data
                    .as_ref()
                    .and_then(|r| r.render_proxies.as_ref());

                select_input(
                    config.proxy_mode,
                    &recording_dir,
                    &source_video_path,
                    proxies,
                    config.output_width,
                )?
            } else {
                return Err(err);
            }
        }
    };

    // Generate proxies on-demand if:
    // 1. No proxy available (using original source), OR
    // 2. Output width > 1920 but only 1080p proxy available (need full proxy)
    let desired_width = config.output_width.unwrap_or(DEFAULT_OUTPUT_WIDTH);
    let needs_full_proxy =
        desired_width > 1920 && matches!(selected_input.origin, proxy::ProxyOrigin::PrimaryProxy);
    let needs_proxy_generation =
        matches!(selected_input.origin, proxy::ProxyOrigin::Original) || needs_full_proxy;

    if matches!(config.proxy_mode, ProxyMode::Auto) && needs_proxy_generation {
        let request = ProxyGenerationRequest {
            generate_1080p: !needs_full_proxy, // Skip 1080p when we specifically need full
            generate_full: desired_width > 1920 || needs_full_proxy,
            full_required: needs_full_proxy,
        };

        if needs_full_proxy {
            info!("Output resolution requires full proxy; generating on demand");
        } else {
            info!("No valid proxy available; generating on demand");
        }
        match ensure_render_proxies(
            &config.session_dir,
            &recording_dir,
            &source_video_path,
            request,
        ) {
            Ok(_) => {
                recording_data = load_recording_data(&config.session_dir)?;
                proxies = recording_data
                    .as_ref()
                    .and_then(|r| r.render_proxies.as_ref());
                selected_input = select_input(
                    config.proxy_mode,
                    &recording_dir,
                    &source_video_path,
                    proxies,
                    config.output_width,
                )?;
            }
            Err(err) => {
                warn!(
                    "On-demand proxy generation failed; falling back to original video: {}",
                    err
                );
            }
        }
    }

    let (target_width, target_height) = compute_target_dimensions(
        selected_input.width,
        selected_input.height,
        config.output_width,
    );

    info!(
        "Using input {:?} ({:?}), target {}x{}",
        selected_input.path, selected_input.origin, target_width, target_height
    );

    let pipeline_metrics = render_cpu(
        &plan,
        &selected_input.path,
        &config.output_path,
        target_width,
        target_height,
        config.realtime,
        config.motion_blur,
    )?;

    if let Some(metrics_path) = config.metrics_json.as_ref() {
        write_render_metrics(
            metrics_path,
            &selected_input,
            &config.output_path,
            target_width,
            target_height,
            plan.video.fps as f64,
            pipeline_metrics,
        )?;
    }

    Ok(())
}

fn render_cpu(
    plan: &RenderPlan,
    input_path: &Path,
    output_path: &Path,
    width: u32,
    height: u32,
    realtime: bool,
    motion_blur: MotionBlurConfig,
) -> Result<scheduler::frame_scheduler::PipelineMetrics> {
    ensure_ffmpeg_initialized()?;

    let fps = plan.video.fps as f64;
    if !(fps.is_finite() && fps > 0.0) {
        return Err(RendererError::Validation(format!(
            "Invalid plan fps {}",
            plan.video.fps
        )));
    }

    if plan.video.width == 0 || plan.video.height == 0 {
        return Err(RendererError::Validation(
            "Plan video dimensions are missing".into(),
        ));
    }
    if width == 0 || height == 0 {
        return Err(RendererError::Validation(
            "Output dimensions must be non-zero".into(),
        ));
    }

    let total_frames = ((plan.playback.output_duration_ms / 1000.0) * fps)
        .ceil()
        .max(0.0) as u32;
    if total_frames == 0 {
        return Err(RendererError::Validation(
            "Plan output duration produces 0 frames".into(),
        ));
    }

    let scale_x = width as f64 / plan.video.width as f64;
    let scale_y = height as f64 / plan.video.height as f64;

    let mut zoom_windows = plan.tracks.zoom_windows.clone();
    for window in &mut zoom_windows {
        for point in &mut window.focus_points {
            point.x *= scale_x;
            point.y *= scale_y;
        }
    }
    zoom_windows.sort_by(|a, b| a.start_ms.total_cmp(&b.start_ms));

    let selected_clicks: Vec<ClickEffectKeyframe> =
        if !plan.decisions.selected_click_effects.is_empty() {
            plan.tracks
                .click_effects
                .iter()
                .enumerate()
                .filter(|(i, _)| plan.decisions.selected_click_effects.contains(i))
                .map(|(_, e)| e.clone())
                .collect()
        } else if plan.decisions.show_click_effects {
            plan.tracks.click_effects.clone()
        } else {
            Vec::new()
        };

    let mut cursor_keyframes: Vec<ClickEffectKeyframe> = selected_clicks
        .into_iter()
        .map(|mut k| {
            k.x *= scale_x;
            k.y *= scale_y;
            k
        })
        .collect();
    cursor_keyframes.sort_by(|a, b| a.video_timestamp_ms.total_cmp(&b.video_timestamp_ms));

    let mut cursor_path_keyframes: Vec<CursorPathKeyframe> = plan
        .decision_input
        .cursor_paths
        .iter()
        .find(|p| p.style == plan.decisions.cursor_style)
        .map(|p| p.keyframes.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|mut k| {
            k.x *= scale_x;
            k.y *= scale_y;
            k
        })
        .collect();
    cursor_path_keyframes.sort_by(|a, b| a.video_timestamp_ms.total_cmp(&b.video_timestamp_ms));

    let keystroke_timeline = compositor::effects::keystrokes::KeystrokeTimeline::new(
        if plan.decisions.show_keystrokes {
            &plan.tracks.keystroke_events
        } else {
            &[]
        },
    );

    scheduler::frame_scheduler::render_cpu_pipelined(
        plan,
        input_path,
        output_path,
        width,
        height,
        fps,
        total_frames,
        realtime,
        zoom_windows,
        cursor_path_keyframes,
        cursor_keyframes,
        keystroke_timeline,
        motion_blur,
        scheduler::frame_scheduler::FrameSchedulerConfig::default(),
    )
}

#[derive(serde::Serialize)]
struct RenderMetricsFile {
    version: u32,
    input: RenderMetricsInput,
    output: RenderMetricsOutput,
    pipeline: scheduler::frame_scheduler::PipelineMetrics,
}

#[derive(serde::Serialize)]
struct RenderMetricsInput {
    path: String,
    origin: String,
    width: u32,
    height: u32,
}

#[derive(serde::Serialize)]
struct RenderMetricsOutput {
    path: String,
    width: u32,
    height: u32,
    fps: f64,
    frames: u32,
}

fn write_render_metrics(
    metrics_path: &Path,
    selected_input: &SelectedInput,
    output_video_path: &Path,
    output_width: u32,
    output_height: u32,
    fps: f64,
    pipeline: scheduler::frame_scheduler::PipelineMetrics,
) -> Result<()> {
    let parent = metrics_path.parent().ok_or_else(|| {
        RendererError::InvalidArgument("metrics_json must have a parent directory".into())
    })?;
    std::fs::create_dir_all(parent)?;

    let input_origin = match selected_input.origin {
        proxy::ProxyOrigin::PrimaryProxy => "render_proxy_1080p",
        proxy::ProxyOrigin::FullProxy => "render_proxy_full",
        proxy::ProxyOrigin::Original => "source",
    }
    .to_string();

    let payload = RenderMetricsFile {
        version: 1,
        input: RenderMetricsInput {
            path: selected_input.path.display().to_string(),
            origin: input_origin,
            width: selected_input.width,
            height: selected_input.height,
        },
        output: RenderMetricsOutput {
            path: output_video_path.display().to_string(),
            width: output_width,
            height: output_height,
            fps,
            frames: pipeline.total_frames,
        },
        pipeline,
    };

    let json = serde_json::to_string_pretty(&payload)?;
    let tmp = metrics_path.with_extension("tmp");
    std::fs::write(&tmp, json.as_bytes())?;
    std::fs::rename(&tmp, metrics_path)?;
    Ok(())
}

fn yuv420p_frame_to_packed_i420_in_place(
    frame: &ffmpeg_next::frame::Video,
    width: u32,
    height: u32,
    out: &mut [u8],
) -> Result<()> {
    if width == 0 || height == 0 {
        return Err(RendererError::InvalidArgument(
            "Frame copy requires non-zero dimensions".into(),
        ));
    }
    if width % 2 != 0 || height % 2 != 0 {
        return Err(RendererError::InvalidArgument(
            "yuv420p copy requires even dimensions".into(),
        ));
    }

    let expected = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(3)
        .saturating_div(2);
    if out.len() != expected {
        return Err(RendererError::InvalidArgument(format!(
            "yuv420p buffer has wrong length (expected {expected}, got {})",
            out.len()
        )));
    }

    if frame.format() != ffmpeg_next::format::Pixel::YUV420P {
        return Err(RendererError::Validation(format!(
            "Decoded frame has unexpected pixel format {:?} (expected YUV420P)",
            frame.format()
        )));
    }

    let y_len = (width as usize).saturating_mul(height as usize);
    let uv_width = width / 2;
    let uv_height = height / 2;
    let uv_len = (uv_width as usize).saturating_mul(uv_height as usize);

    copy_plane(frame, 0, width, height, &mut out[0..y_len], "Y")?;
    copy_plane(
        frame,
        1,
        uv_width,
        uv_height,
        &mut out[y_len..y_len + uv_len],
        "U",
    )?;
    copy_plane(
        frame,
        2,
        uv_width,
        uv_height,
        &mut out[y_len + uv_len..y_len + uv_len + uv_len],
        "V",
    )?;

    Ok(())
}

fn copy_plane(
    frame: &ffmpeg_next::frame::Video,
    plane: usize,
    width: u32,
    height: u32,
    out: &mut [u8],
    name: &str,
) -> Result<()> {
    let expected = (width as usize).saturating_mul(height as usize);
    if out.len() != expected {
        return Err(RendererError::InvalidArgument(format!(
            "Plane {name} output slice has wrong length (expected {expected}, got {})",
            out.len()
        )));
    }

    let stride = frame.stride(plane);
    let data = frame.data(plane);
    let row_bytes = width as usize;
    if stride < row_bytes {
        return Err(RendererError::Validation(format!(
            "Plane {name} stride too small (stride {stride}, row_bytes {row_bytes})"
        )));
    }
    if data.len() < stride.saturating_mul(height as usize) {
        return Err(RendererError::Validation(format!(
            "Plane {name} data buffer shorter than expected"
        )));
    }

    if stride == row_bytes {
        out.copy_from_slice(&data[..row_bytes.saturating_mul(height as usize)]);
        return Ok(());
    }

    for y in 0..height as usize {
        let src_row = &data[y * stride..y * stride + row_bytes];
        let dst_row = &mut out[y * row_bytes..(y + 1) * row_bytes];
        dst_row.copy_from_slice(src_row);
    }

    Ok(())
}

fn resolve_video_path(path_str: &str, session_dir: &Path) -> PathBuf {
    let path = PathBuf::from(path_str);
    if path.is_absolute() {
        return path;
    }
    if path.exists() {
        return path;
    }

    session_dir.join(path)
}

#[cfg(test)]
mod resolve_video_path_tests {
    use super::resolve_video_path;
    use std::path::{Path, PathBuf};

    fn unique_tmp_dir() -> PathBuf {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        PathBuf::from(format!(
            "target/tmp-polished-renderer/resolve_video_path/{}-{}",
            std::process::id(),
            now.as_nanos()
        ))
    }

    #[test]
    fn prefers_existing_relative_path_over_session_join() {
        let session_dir = unique_tmp_dir();
        let recording_dir = session_dir.join("recording");
        std::fs::create_dir_all(&recording_dir).unwrap();
        let video_path = recording_dir.join("recording_full.mp4");
        std::fs::write(&video_path, []).unwrap();

        let path_str = video_path.display().to_string();
        let resolved = resolve_video_path(&path_str, &session_dir);
        assert_eq!(resolved, video_path);
    }

    #[test]
    fn joins_when_relative_path_does_not_exist() {
        let session_dir = unique_tmp_dir();
        let recording_dir = session_dir.join("recording");
        std::fs::create_dir_all(&recording_dir).unwrap();
        let expected = recording_dir.join("recording_full.mp4");
        std::fs::write(&expected, []).unwrap();

        let resolved = resolve_video_path("recording/recording_full.mp4", Path::new(&session_dir));
        assert_eq!(resolved, expected);
    }
}
