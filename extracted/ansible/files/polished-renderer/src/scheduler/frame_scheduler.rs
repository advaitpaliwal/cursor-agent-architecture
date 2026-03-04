use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use crossbeam_channel as channel;
use ffmpeg_next as ffmpeg;
use serde::Serialize;
use tracing::{info, warn};

use crate::compositor::cpu::CpuCompositor;
use crate::compositor::effects::{cursor, keystrokes};
use crate::compositor::i420_frame::I420Frame;
use crate::error::{RendererError, Result};
use crate::video::{verify_output_video, OutputVideoExpectations, VideoDecoder, VideoEncoder};
use crate::{
    yuv420p_frame_to_packed_i420_in_place, MotionBlurConfig, PlaybackSegment, RenderPlan,
    ZoomWindow,
};

#[derive(Debug, Clone, Copy)]
pub struct FrameSchedulerConfig {
    pub decode_buffer_size: usize,
    pub encode_buffer_size: usize,
}

impl Default for FrameSchedulerConfig {
    fn default() -> Self {
        Self {
            decode_buffer_size: 10,
            encode_buffer_size: 6,
        }
    }
}

struct DecodedPacket {
    frame_idx: u32,
    output_time_ms: f64,
    i420: Vec<u8>,
}

struct EncodedPacket {
    frame_idx: u32,
    i420: Vec<u8>,
}

#[derive(Default)]
struct PipelineStats {
    decoded_frames: AtomicU64,
    decoded_ns: AtomicU64,
    composed_frames: AtomicU64,
    composed_ns: AtomicU64,
    encoded_frames: AtomicU64,
    encoded_ns: AtomicU64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PipelineMetrics {
    pub total_frames: u32,
    pub wall_ms: f64,
    pub render_fps: f64,
    pub decode_ms: f64,
    pub compose_ms: f64,
    pub encode_ms: f64,
    pub decode_fps: f64,
    pub compose_fps: f64,
    pub encode_fps: f64,
}

pub fn render_cpu_pipelined(
    plan: &RenderPlan,
    input_path: &Path,
    output_path: &Path,
    width: u32,
    height: u32,
    fps: f64,
    total_frames: u32,
    realtime: bool,
    zoom_windows: Vec<ZoomWindow>,
    cursor_path_keyframes: Vec<crate::CursorPathKeyframe>,
    cursor_click_keyframes: Vec<crate::ClickEffectKeyframe>,
    keystroke_timeline: keystrokes::KeystrokeTimeline,
    motion_blur: MotionBlurConfig,
    scheduler_config: FrameSchedulerConfig,
) -> Result<PipelineMetrics> {
    let global_start = Instant::now();
    let frame_duration_ms = 1000.0 / fps;
    let max_source_time_ms = (plan.video.source_duration_ms - frame_duration_ms).max(0.0);

    let decode_capacity = scheduler_config.decode_buffer_size.max(1);
    let encode_capacity = scheduler_config.encode_buffer_size.max(1);

    let rgba_len = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(3)
        .saturating_div(2);
    if rgba_len == 0 {
        return Err(RendererError::Validation(
            "Output dimensions produce an empty frame".into(),
        ));
    }

    let (free_decode_tx, free_decode_rx) = channel::bounded::<Vec<u8>>(decode_capacity);
    let (free_output_tx, free_output_rx) = channel::bounded::<Vec<u8>>(encode_capacity);

    for _ in 0..decode_capacity {
        free_decode_tx
            .send(vec![0u8; rgba_len])
            .map_err(|_| RendererError::Other("Failed to seed decode buffer pool".into()))?;
    }
    for _ in 0..encode_capacity {
        free_output_tx
            .send(vec![0u8; rgba_len])
            .map_err(|_| RendererError::Other("Failed to seed output buffer pool".into()))?;
    }

    let (decode_tx, decode_rx) = channel::bounded::<DecodedPacket>(decode_capacity);
    let (encode_tx, encode_rx) = channel::bounded::<EncodedPacket>(encode_capacity);

    let input_path = input_path.to_path_buf();
    let output_path = output_path.to_path_buf();

    let segments: Vec<PlaybackSegment> = plan.playback.segments.clone();
    validate_playback_segments(&segments, plan.playback.output_duration_ms)?;
    let stats = Arc::new(PipelineStats::default());

    let decode_handle = spawn_decode_thread(
        input_path,
        width,
        height,
        fps,
        total_frames,
        max_source_time_ms,
        segments,
        free_decode_rx,
        decode_tx,
        Arc::clone(&stats),
    )?;

    let encode_handle = spawn_encode_thread(
        output_path,
        width,
        height,
        fps,
        realtime,
        encode_rx,
        free_output_tx.clone(),
        Arc::clone(&stats),
    )?;

    let compose_result = compose_loop(
        width,
        height,
        fps,
        frame_duration_ms,
        &zoom_windows,
        &cursor_path_keyframes,
        &cursor_click_keyframes,
        &keystroke_timeline,
        motion_blur,
        decode_rx,
        free_decode_tx,
        encode_tx,
        free_output_rx,
        plan.decisions.show_keystrokes,
        Arc::clone(&stats),
    );

    if let Err(ref err) = compose_result {
        warn!("Compose loop failed: {err}");
    }

    join_thread(decode_handle, "decode")?;
    join_thread(encode_handle, "encode")?;

    compose_result?;

    let wall_ns = global_start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
    let wall_s = (wall_ns as f64) / 1e9;
    let decoded_frames = stats.decoded_frames.load(Ordering::Relaxed) as f64;
    let composed_frames = stats.composed_frames.load(Ordering::Relaxed) as f64;
    let encoded_frames = stats.encoded_frames.load(Ordering::Relaxed) as f64;

    let decode_s = (stats.decoded_ns.load(Ordering::Relaxed) as f64) / 1e9;
    let compose_s = (stats.composed_ns.load(Ordering::Relaxed) as f64) / 1e9;
    let encode_s = (stats.encoded_ns.load(Ordering::Relaxed) as f64) / 1e9;

    let render_fps = (total_frames as f64) / wall_s.max(1e-9);
    let decode_fps = decoded_frames / decode_s.max(1e-9);
    let compose_fps = composed_frames / compose_s.max(1e-9);
    let encode_fps = encoded_frames / encode_s.max(1e-9);

    info!(
        "Render complete: {} frames in {:.2}s (~{:.1} fps). stage_fps: decode={:.1}, composite={:.1}, encode={:.1}",
        total_frames,
        wall_s,
        render_fps,
        decode_fps,
        compose_fps,
        encode_fps,
    );

    Ok(PipelineMetrics {
        total_frames,
        wall_ms: wall_s * 1000.0,
        render_fps,
        decode_ms: decode_s * 1000.0,
        compose_ms: compose_s * 1000.0,
        encode_ms: encode_s * 1000.0,
        decode_fps,
        compose_fps,
        encode_fps,
    })
}

fn validate_playback_segments(segments: &[PlaybackSegment], output_duration_ms: f64) -> Result<()> {
    if segments.is_empty() {
        return Ok(());
    }

    if !(output_duration_ms.is_finite() && output_duration_ms >= 0.0) {
        return Err(RendererError::Validation(format!(
            "Playback output duration is invalid: {output_duration_ms}"
        )));
    }

    const EPS: f64 = 1e-6;

    let first = &segments[0];
    if !(first.output_start_ms.is_finite() && first.output_start_ms.abs() <= EPS) {
        return Err(RendererError::Validation(format!(
            "Playback segments must start at 0ms (got {})",
            first.output_start_ms
        )));
    }

    let last = segments
        .last()
        .ok_or_else(|| RendererError::Validation("Playback segments are empty".into()))?;
    if (last.output_end_ms - output_duration_ms).abs() > 1e-3 {
        return Err(RendererError::Validation(format!(
            "Playback segments end at {}ms but output duration is {}ms",
            last.output_end_ms, output_duration_ms
        )));
    }

    let mut prev_output_end = first.output_end_ms;
    for (idx, seg) in segments.iter().enumerate() {
        if !(seg.output_start_ms.is_finite() && seg.output_end_ms.is_finite()) {
            return Err(RendererError::Validation(format!(
                "Playback segment {idx} has non-finite output times"
            )));
        }
        if seg.output_end_ms + EPS < seg.output_start_ms {
            return Err(RendererError::Validation(format!(
                "Playback segment {idx} has negative output duration ({}..{})",
                seg.output_start_ms, seg.output_end_ms
            )));
        }
        if !(seg.source_start_ms.is_finite()
            && seg.source_end_ms.is_finite()
            && seg.source_duration_ms.is_finite())
        {
            return Err(RendererError::Validation(format!(
                "Playback segment {idx} has non-finite source times"
            )));
        }
        if seg.source_end_ms + EPS < seg.source_start_ms {
            return Err(RendererError::Validation(format!(
                "Playback segment {idx} has negative source duration ({}..{})",
                seg.source_start_ms, seg.source_end_ms
            )));
        }
        if !(seg.playback_rate.is_finite() && seg.playback_rate > 0.0) {
            return Err(RendererError::Validation(format!(
                "Playback segment {idx} has invalid playback rate {}",
                seg.playback_rate
            )));
        }

        if idx == 0 {
            continue;
        }

        let delta = seg.output_start_ms - prev_output_end;
        if delta < -EPS {
            return Err(RendererError::Validation(format!(
                "Playback segment {idx} overlaps previous segment (delta {delta}ms)"
            )));
        }
        if delta > EPS {
            return Err(RendererError::Validation(format!(
                "Playback segment {idx} introduces a gap (delta {delta}ms)"
            )));
        }

        prev_output_end = seg.output_end_ms;
    }

    Ok(())
}

fn spawn_decode_thread(
    input_path: PathBuf,
    width: u32,
    height: u32,
    fps: f64,
    total_frames: u32,
    max_source_time_ms: f64,
    segments: Vec<PlaybackSegment>,
    free_decode_rx: channel::Receiver<Vec<u8>>,
    decode_tx: channel::Sender<DecodedPacket>,
    stats: Arc<PipelineStats>,
) -> Result<thread::JoinHandle<Result<()>>> {
    thread::Builder::new()
        .name("polished-renderer-decode".into())
        .spawn(move || {
            let mut decoder = VideoDecoder::open(&input_path, width, height)?;
            let mut yuv_frame = ffmpeg::frame::Video::empty();
            let mut decoded_frame = ffmpeg::frame::Video::empty();
            let mut segment_idx = 0usize;
            let mut last_decoded_timestamp_us: Option<i64> = None;
            let frame_duration_ms = 1000.0 / fps;
            let max_forward_decode_us = (frame_duration_ms * 1000.0 * 3.0)
                .round()
                .clamp(0.0, i64::MAX as f64) as i64;

            for frame_idx in 0..total_frames {
                let t0 = Instant::now();
                let output_time_ms = (frame_idx as f64 / fps) * 1000.0;
                let segment = if segments.is_empty() {
                    None
                } else {
                    while segment_idx + 1 < segments.len()
                        && output_time_ms > segments[segment_idx].output_end_ms
                    {
                        segment_idx += 1;
                    }

                    segments.get(segment_idx).and_then(|seg| {
                        if output_time_ms >= seg.output_start_ms
                            && output_time_ms <= seg.output_end_ms
                        {
                            Some(seg)
                        } else {
                            None
                        }
                    })
                };

                let source_time_ms = if let Some(seg) = segment {
                    let output_offset = output_time_ms - seg.output_start_ms;
                    let source_offset = output_offset * seg.playback_rate;
                    seg.source_start_ms + source_offset
                } else if let Some(last) = segments.last() {
                    last.source_end_ms
                } else {
                    output_time_ms
                };
                let decode_time_ms = source_time_ms.clamp(0.0, max_source_time_ms);
                let target_timestamp_us = (decode_time_ms * 1000.0).round() as i64;

                let mut i420_buf = match free_decode_rx.recv() {
                    Ok(buf) => buf,
                    Err(_) => return Ok(()),
                };

                let should_decode_forward = match (segment, last_decoded_timestamp_us) {
                    (Some(seg), Some(last_ts)) => {
                        let rate_near_one = (seg.playback_rate - 1.0).abs() <= 0.01;
                        let delta_us = target_timestamp_us - last_ts;
                        rate_near_one && delta_us > 0 && delta_us <= max_forward_decode_us
                    }
                    _ => false,
                };

                let decoded = if should_decode_forward {
                    decoder.decode_yuv420p_frame_from_current_time_ms(
                        decode_time_ms,
                        &mut yuv_frame,
                        &mut decoded_frame,
                    )?
                } else {
                    decoder.decode_yuv420p_frame_at_time_ms(
                        decode_time_ms,
                        &mut yuv_frame,
                        &mut decoded_frame,
                    )?
                };

                let decoded = match decoded {
                    Some(info) => info,
                    None => {
                        return Err(RendererError::Validation(format!(
                            "Failed to decode frame at source time {:.2}ms",
                            decode_time_ms
                        )));
                    }
                };

                last_decoded_timestamp_us = decoded.timestamp_us;
                yuv420p_frame_to_packed_i420_in_place(&yuv_frame, width, height, &mut i420_buf)?;

                let elapsed = t0.elapsed();
                stats.decoded_frames.fetch_add(1, Ordering::Relaxed);
                stats.decoded_ns.fetch_add(
                    elapsed.as_nanos().min(u128::from(u64::MAX)) as u64,
                    Ordering::Relaxed,
                );

                if decode_tx
                    .send(DecodedPacket {
                        frame_idx,
                        output_time_ms,
                        i420: i420_buf,
                    })
                    .is_err()
                {
                    return Ok(());
                }
            }

            Ok(())
        })
        .map_err(|err| RendererError::Other(format!("Failed to spawn decode thread: {err}")))
}

fn spawn_encode_thread(
    output_path: PathBuf,
    width: u32,
    height: u32,
    fps: f64,
    realtime: bool,
    encode_rx: channel::Receiver<EncodedPacket>,
    free_output_tx: channel::Sender<Vec<u8>>,
    stats: Arc<PipelineStats>,
) -> Result<thread::JoinHandle<Result<()>>> {
    thread::Builder::new()
        .name("polished-renderer-encode".into())
        .spawn(move || {
            let mut encoder = VideoEncoder::new(&output_path, width, height, fps, realtime)?;
            let mut expected_frame_idx = 0u32;

            while let Ok(packet) = encode_rx.recv() {
                if packet.frame_idx != expected_frame_idx {
                    return Err(RendererError::Validation(format!(
                        "Encoder received out-of-order frame {} (expected {})",
                        packet.frame_idx, expected_frame_idx
                    )));
                }
                expected_frame_idx += 1;

                let t0 = Instant::now();
                encoder.write_frame_yuv420p(&packet.i420)?;
                let elapsed = t0.elapsed();
                stats.encoded_frames.fetch_add(1, Ordering::Relaxed);
                stats.encoded_ns.fetch_add(
                    elapsed.as_nanos().min(u128::from(u64::MAX)) as u64,
                    Ordering::Relaxed,
                );
                if free_output_tx.send(packet.i420).is_err() {
                    // The compose loop has exited (receiver dropped), so we can't recycle output
                    // buffers. That's fine; we'll just drop them.
                }
            }

            encoder.finish()?;
            verify_output_video(
                &output_path,
                OutputVideoExpectations {
                    width,
                    height,
                    fps,
                    frames: expected_frame_idx,
                },
            )?;
            Ok(())
        })
        .map_err(|err| RendererError::Other(format!("Failed to spawn encode thread: {err}")))
}

#[allow(clippy::too_many_arguments)]
fn compose_loop(
    width: u32,
    height: u32,
    fps: f64,
    frame_duration_ms: f64,
    zoom_windows: &[ZoomWindow],
    cursor_path_keyframes: &[crate::CursorPathKeyframe],
    cursor_click_keyframes: &[crate::ClickEffectKeyframe],
    keystroke_timeline: &keystrokes::KeystrokeTimeline,
    motion_blur: MotionBlurConfig,
    decode_rx: channel::Receiver<DecodedPacket>,
    free_decode_tx: channel::Sender<Vec<u8>>,
    encode_tx: channel::Sender<EncodedPacket>,
    free_output_rx: channel::Receiver<Vec<u8>>,
    enable_keystrokes: bool,
    stats: Arc<PipelineStats>,
) -> Result<()> {
    let mut compositor = CpuCompositor::new(width, height, enable_keystrokes, motion_blur)?;

    let mut rendered = 0u32;
    while let Ok(packet) = decode_rx.recv() {
        let t0 = Instant::now();
        let mut output_buf = match free_output_rx.recv() {
            Ok(buf) => buf,
            Err(_) => return Ok(()),
        };

        let prev_output_time_ms = if packet.frame_idx == 0 {
            packet.output_time_ms
        } else {
            ((packet.frame_idx - 1) as f64 / fps) * 1000.0
        };

        let source_frame = I420Frame {
            width,
            height,
            data: packet.i420,
        };

        let cursor_state = cursor::compute_cursor_state_from_path(
            packet.output_time_ms,
            cursor_path_keyframes,
            cursor_click_keyframes,
        );
        let prev_cursor_state = cursor::compute_cursor_state_from_path(
            prev_output_time_ms,
            cursor_path_keyframes,
            cursor_click_keyframes,
        );
        let keystroke_state = keystroke_timeline.state_at(packet.output_time_ms);

        compositor.render_frame(
            &source_frame,
            zoom_windows,
            packet.output_time_ms,
            frame_duration_ms,
            cursor_state,
            prev_cursor_state,
            keystroke_state,
        )?;

        compositor.swap_output_buffer(&mut output_buf)?;

        let elapsed = t0.elapsed();
        stats.composed_frames.fetch_add(1, Ordering::Relaxed);
        stats.composed_ns.fetch_add(
            elapsed.as_nanos().min(u128::from(u64::MAX)) as u64,
            Ordering::Relaxed,
        );

        if encode_tx
            .send(EncodedPacket {
                frame_idx: packet.frame_idx,
                i420: output_buf,
            })
            .is_err()
        {
            return Ok(());
        }

        if free_decode_tx.send(source_frame.data).is_err() {
            // Decode loop has exited (receiver dropped), so there's no need to recycle buffers.
        }

        rendered += 1;
        if rendered % 600 == 0 {
            let secs = (rendered as f64 / fps).round();
            info!("Rendered ~{secs:.0}s ({rendered} frames)");
        }
    }

    Ok(())
}

fn join_thread(handle: thread::JoinHandle<Result<()>>, name: &'static str) -> Result<()> {
    handle
        .join()
        .map_err(|_| RendererError::Other(format!("{name} thread panicked")))??;
    Ok(())
}
