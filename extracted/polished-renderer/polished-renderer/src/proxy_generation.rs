use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;
use tracing::{info, warn};

use crate::config::{
    PROXY_EXPECTED_FPS, PROXY_FULL_FILENAME, PROXY_PRIMARY_FILENAME, PROXY_PROFILE_VERSION,
};
use crate::error::{RendererError, Result};
use crate::plan::types::{ProxySourceMetadata, RenderProxiesMetadata, RenderProxyArtifact};

const PROXY_CRF: &str = "17";
const PROXY_PRESET: &str = "veryfast";
const PROXY_KEYINT: u32 = 1;

const LOCK_FILENAME: &str = "render-proxies.lock";
const METADATA_FILENAME: &str = "render-proxies.json";
const SLOW_PATH_MARKER_FILENAME: &str = "render-proxies.slow-path.json";

#[derive(Debug, Clone, Copy)]
pub struct ProxyGenerationRequest {
    pub generate_1080p: bool,
    pub generate_full: bool,
    pub full_required: bool,
}

pub fn ensure_render_proxies(
    session_dir: &Path,
    recording_dir: &Path,
    source_video_path: &Path,
    request: ProxyGenerationRequest,
) -> Result<Option<RenderProxiesMetadata>> {
    if !request.generate_1080p && !request.generate_full {
        return Ok(None);
    }

    if !recording_dir.exists() {
        fs::create_dir_all(recording_dir)?;
    }

    let lock_path = recording_dir.join(LOCK_FILENAME);
    let _lock_guard = acquire_lock(&lock_path)?;

    let start = Instant::now();
    let source = probe_source_metadata(source_video_path)?;

    let generated_at_epoch_ms = epoch_ms()?;
    let mut artifacts: Vec<RenderProxyArtifact> = Vec::new();
    let mut fatal_error: Option<String> = None;

    if request.generate_1080p {
        let artifact = generate_proxy_artifact(
            "render_proxy_1080p",
            recording_dir,
            source_video_path,
            ProxySpec::Proxy1080p {
                source_width: source.width,
            },
        );
        if artifact.status != "success" {
            fatal_error = Some(format!(
                "Failed to generate render_proxy_1080p: {}",
                artifact
                    .error
                    .as_deref()
                    .unwrap_or("unknown proxy generation error")
            ));
        }
        artifacts.push(artifact);
    }

    if request.generate_full {
        let artifact = generate_proxy_artifact(
            "render_proxy_full",
            recording_dir,
            source_video_path,
            ProxySpec::ProxyFull,
        );
        if artifact.status != "success" && request.full_required && fatal_error.is_none() {
            fatal_error = Some(format!(
                "Failed to generate render_proxy_full: {}",
                artifact
                    .error
                    .as_deref()
                    .unwrap_or("unknown proxy generation error")
            ));
        }
        artifacts.push(artifact);
    }

    let metadata = RenderProxiesMetadata {
        profile_version: PROXY_PROFILE_VERSION.to_string(),
        generated_at_epoch_ms,
        source,
        artifacts,
    };

    write_proxy_metadata(recording_dir, &metadata)?;
    upsert_recording_data_render_proxies(session_dir, &metadata)?;
    write_slow_path_marker(recording_dir, &metadata, start.elapsed())?;

    if let Some(message) = fatal_error {
        return Err(RendererError::Other(message));
    }

    Ok(Some(metadata))
}

fn epoch_ms() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|err| RendererError::Other(format!("System time error: {err}")))?
        .as_millis() as u64)
}

fn acquire_lock(lock_path: &Path) -> Result<LockGuard> {
    let start = Instant::now();

    loop {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(lock_path)
        {
            Ok(file) => {
                drop(file);
                return Ok(LockGuard {
                    path: lock_path.to_path_buf(),
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                if is_lock_stale(lock_path, Duration::from_secs(30 * 60))? {
                    warn!("Proxy lock appears stale; removing {}", lock_path.display());
                    let _ = fs::remove_file(lock_path);
                    continue;
                }

                if start.elapsed() > Duration::from_secs(10 * 60) {
                    return Err(RendererError::Other(format!(
                        "Timed out waiting for proxy generation lock at {}",
                        lock_path.display()
                    )));
                }

                thread::sleep(Duration::from_secs(1));
            }
            Err(err) => return Err(err.into()),
        }
    }
}

fn is_lock_stale(lock_path: &Path, max_age: Duration) -> Result<bool> {
    let meta = match fs::metadata(lock_path) {
        Ok(m) => m,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err.into()),
    };
    let modified = match meta.modified() {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };
    let age = match SystemTime::now().duration_since(modified) {
        Ok(d) => d,
        Err(_) => return Ok(false),
    };
    Ok(age > max_age)
}

struct LockGuard {
    path: PathBuf,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Debug, Clone, Copy)]
enum ProxySpec {
    Proxy1080p { source_width: u32 },
    ProxyFull,
}

impl ProxySpec {
    fn output_filename(self) -> &'static str {
        match self {
            ProxySpec::Proxy1080p { .. } => PROXY_PRIMARY_FILENAME,
            ProxySpec::ProxyFull => PROXY_FULL_FILENAME,
        }
    }

    fn scale_filter(self) -> Option<String> {
        match self {
            ProxySpec::Proxy1080p { source_width } => {
                if source_width > 1920 {
                    Some("scale=1920:-2:flags=lanczos".to_string())
                } else {
                    None
                }
            }
            ProxySpec::ProxyFull => None,
        }
    }
}

fn generate_proxy_artifact(
    name: &str,
    recording_dir: &Path,
    source_video_path: &Path,
    spec: ProxySpec,
) -> RenderProxyArtifact {
    let output_path = recording_dir.join(spec.output_filename());
    let relative_path = PathBuf::from(spec.output_filename())
        .to_string_lossy()
        .to_string();

    let mut artifact = RenderProxyArtifact {
        name: name.to_string(),
        path: relative_path,
        width: 0,
        height: 0,
        fps: PROXY_EXPECTED_FPS,
        codec: "h264".to_string(),
        profile: PROXY_PROFILE_VERSION.to_string(),
        keyint: PROXY_KEYINT,
        status: "failed".to_string(),
        elapsed_ms: 0,
        error: None,
    };

    let start = Instant::now();
    let scale_filter = spec.scale_filter();
    let mut filters = vec![format!("fps={PROXY_EXPECTED_FPS}")];
    if let Some(scale) = scale_filter {
        filters.push(scale);
    }

    let args: Vec<String> = vec![
        "-y".into(),
        "-i".into(),
        source_video_path.to_string_lossy().to_string(),
        "-an".into(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        PROXY_PRESET.into(),
        "-crf".into(),
        PROXY_CRF.into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-profile:v".into(),
        "high".into(),
        "-x264-params".into(),
        "keyint=1:min-keyint=1:scenecut=0:bframes=0".into(),
        "-vsync".into(),
        "1".into(),
        "-r".into(),
        PROXY_EXPECTED_FPS.to_string(),
        "-movflags".into(),
        "+faststart".into(),
        "-tune".into(),
        "fastdecode".into(),
        "-vf".into(),
        filters.join(","),
        output_path.to_string_lossy().to_string(),
    ];

    info!("Generating {name} via ffmpeg");

    match run_ffmpeg_command(&args) {
        Ok(()) => match probe_video_dimensions(&output_path) {
            Ok((w, h)) => {
                artifact.width = w;
                artifact.height = h;
                artifact.status = "success".to_string();
            }
            Err(err) => {
                artifact.error = Some(err.to_string());
            }
        },
        Err(err) => {
            artifact.error = Some(err.to_string());
        }
    }

    artifact.elapsed_ms = start.elapsed().as_millis() as u64;
    artifact
}

fn run_ffmpeg_command(args: &[String]) -> Result<()> {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    cmd.args(args);

    let output = cmd.output()?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(RendererError::Other(format!(
        "ffmpeg exited with {}: {}",
        output.status, stderr
    )))
}

fn probe_video_dimensions(path: &Path) -> Result<(u32, u32)> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0:s=x",
        ])
        .arg(path)
        .output()?;

    if !output.status.success() {
        return Err(RendererError::Ffprobe {
            path: path.to_path_buf(),
            message: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    let mut parts = trimmed.split('x');
    let w: u32 = parts
        .next()
        .ok_or_else(|| RendererError::Validation("ffprobe missing width".into()))?
        .parse()
        .map_err(|_| RendererError::Validation("ffprobe returned invalid width".into()))?;
    let h: u32 = parts
        .next()
        .ok_or_else(|| RendererError::Validation("ffprobe missing height".into()))?
        .parse()
        .map_err(|_| RendererError::Validation("ffprobe returned invalid height".into()))?;
    Ok((w, h))
}

fn probe_source_metadata(path: &Path) -> Result<ProxySourceMetadata> {
    let dims = probe_video_dimensions(path)?;
    let fps = probe_video_fps(path)?.unwrap_or(PROXY_EXPECTED_FPS as f64);
    let duration_ms = probe_video_duration_ms(path)?;

    Ok(ProxySourceMetadata {
        width: dims.0,
        height: dims.1,
        duration_ms,
        fps: fps.round().max(0.0) as u32,
    })
}

fn probe_video_fps(path: &Path) -> Result<Option<f64>> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=r_frame_rate",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_frame_rate(stdout.trim()).ok())
}

fn parse_frame_rate(value: &str) -> std::result::Result<f64, ()> {
    if value.contains('/') {
        let mut parts = value.split('/');
        let num: f64 = parts.next().ok_or(())?.parse().map_err(|_| ())?;
        let den: f64 = parts.next().ok_or(())?.parse().map_err(|_| ())?;
        if den.abs() < f64::EPSILON {
            return Err(());
        }
        Ok(num / den)
    } else {
        value.parse::<f64>().map_err(|_| ())
    }
}

fn probe_video_duration_ms(path: &Path) -> Result<u64> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()?;
    if !output.status.success() {
        return Err(RendererError::Ffprobe {
            path: path.to_path_buf(),
            message: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let secs: f64 = stdout.trim().parse().map_err(|_| {
        RendererError::Validation(format!(
            "ffprobe returned invalid duration for {}",
            path.display()
        ))
    })?;
    if !secs.is_finite() || secs < 0.0 {
        return Err(RendererError::Validation(format!(
            "ffprobe returned invalid duration for {}",
            path.display()
        )));
    }
    Ok((secs * 1000.0).floor().max(0.0) as u64)
}

fn write_proxy_metadata(recording_dir: &Path, metadata: &RenderProxiesMetadata) -> Result<()> {
    let path = recording_dir.join(METADATA_FILENAME);
    let json = serde_json::to_string_pretty(metadata)?;
    fs::write(path, json)?;
    Ok(())
}

fn upsert_recording_data_render_proxies(
    session_dir: &Path,
    metadata: &RenderProxiesMetadata,
) -> Result<()> {
    let recording_data_path = session_dir.join("recording").join("recording-data.json");
    if !recording_data_path.exists() {
        return Ok(());
    }

    let raw = fs::read_to_string(&recording_data_path)?;
    let mut value: Value = serde_json::from_str(&raw)?;

    if let Some(obj) = value.as_object_mut() {
        let version = obj.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
        if version < 3 {
            obj.insert("version".to_string(), Value::from(3));
        }
        obj.insert("renderProxies".to_string(), serde_json::to_value(metadata)?);
    }

    let updated = serde_json::to_string_pretty(&value)?;
    fs::write(&recording_data_path, updated)?;
    Ok(())
}

fn write_slow_path_marker(
    recording_dir: &Path,
    metadata: &RenderProxiesMetadata,
    elapsed: Duration,
) -> Result<()> {
    let marker_path = recording_dir.join(SLOW_PATH_MARKER_FILENAME);
    let value = serde_json::json!({
        "generatedAtEpochMs": metadata.generated_at_epoch_ms,
        "profileVersion": metadata.profile_version,
        "elapsedMs": elapsed.as_millis(),
        "reason": "on_demand_proxy_generation",
        "renderer": "polished-renderer",
    });
    fs::write(marker_path, serde_json::to_string_pretty(&value)?)?;
    Ok(())
}
