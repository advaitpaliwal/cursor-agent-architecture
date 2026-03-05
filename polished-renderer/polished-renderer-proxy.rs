use std::path::{Path, PathBuf};
use std::process::Command;

use clap::ValueEnum;
use serde::Deserialize;

use crate::config::{
    PROXY_EXPECTED_FPS, PROXY_FULL_FILENAME, PROXY_PRIMARY_FILENAME, PROXY_PROFILE_VERSION,
};
use crate::error::{RendererError, Result};
use crate::plan::types::{RenderProxiesMetadata, RenderProxyArtifact};
use tracing::warn;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ProxyMode {
    Auto,
    #[value(name = "1080p")]
    Proxy1080p,
    #[value(name = "full")]
    ProxyFull,
    #[value(name = "none")]
    None,
}

#[derive(Debug, Clone, Copy)]
pub enum ProxyOrigin {
    PrimaryProxy,
    FullProxy,
    Original,
}

#[derive(Debug, Clone)]
pub struct SelectedInput {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub fps: Option<f64>,
    pub origin: ProxyOrigin,
}

pub fn select_input(
    mode: ProxyMode,
    recording_dir: &Path,
    original_video_path: &Path,
    proxies: Option<&RenderProxiesMetadata>,
    desired_output_width: Option<u32>,
) -> Result<SelectedInput> {
    let metadata_ok = proxies
        .map(|m| m.profile_version == PROXY_PROFILE_VERSION)
        .unwrap_or(false);

    if let Some(meta) = proxies {
        if !metadata_ok {
            warn!(
                "Proxy metadata version mismatch (found {}, expected {})",
                meta.profile_version, PROXY_PROFILE_VERSION
            );
        }
    }

    let mut candidates = Vec::new();

    if metadata_ok {
        if let Some(proxy) = find_artifact(proxies, "render_proxy_1080p") {
            candidates.push(ProxyCandidate {
                origin: ProxyOrigin::PrimaryProxy,
                artifact_status: Some(proxy.status.clone()),
                path: resolve_artifact_path(recording_dir, &proxy.path),
            });
        }
        if let Some(proxy) = find_artifact(proxies, "render_proxy_full") {
            candidates.push(ProxyCandidate {
                origin: ProxyOrigin::FullProxy,
                artifact_status: Some(proxy.status.clone()),
                path: resolve_artifact_path(recording_dir, &proxy.path),
            });
        }
    } else {
        candidates.push(ProxyCandidate {
            origin: ProxyOrigin::PrimaryProxy,
            artifact_status: None,
            path: recording_dir.join(PROXY_PRIMARY_FILENAME),
        });
        candidates.push(ProxyCandidate {
            origin: ProxyOrigin::FullProxy,
            artifact_status: None,
            path: recording_dir.join(PROXY_FULL_FILENAME),
        });
    }

    let chosen = match mode {
        ProxyMode::None => None,
        ProxyMode::Proxy1080p => candidates
            .iter()
            .find(|c| matches!(c.origin, ProxyOrigin::PrimaryProxy)),
        ProxyMode::ProxyFull => candidates
            .iter()
            .find(|c| matches!(c.origin, ProxyOrigin::FullProxy)),
        ProxyMode::Auto => auto_select_proxy(&candidates, desired_output_width.unwrap_or(u32::MAX)),
    };

    if let Some(candidate) = chosen {
        if candidate.path.exists() {
            let validation = validate_proxy_candidate(candidate)?;
            return Ok(validation);
        }
        if !matches!(mode, ProxyMode::Auto) {
            return Err(RendererError::Validation(format!(
                "Proxy {} not found at {}",
                proxy_label(candidate.origin),
                candidate.path.display()
            )));
        }
    }

    warn!(
        "Using original video at {} (no valid proxy available)",
        original_video_path.display()
    );

    let fallback_probe = probe_video(original_video_path)?;
    Ok(SelectedInput {
        path: original_video_path.to_path_buf(),
        width: fallback_probe.width,
        height: fallback_probe.height,
        fps: fallback_probe.fps,
        origin: ProxyOrigin::Original,
    })
}

fn auto_select_proxy<'a>(
    candidates: &'a [ProxyCandidate],
    desired_width: u32,
) -> Option<&'a ProxyCandidate> {
    let mut ordered = Vec::new();

    if desired_width > 1920 {
        if let Some(full) = candidates
            .iter()
            .find(|c| matches!(c.origin, ProxyOrigin::FullProxy))
        {
            ordered.push(full);
        }
    }

    if let Some(primary) = candidates
        .iter()
        .find(|c| matches!(c.origin, ProxyOrigin::PrimaryProxy))
    {
        ordered.push(primary);
    }

    if let Some(full) = candidates
        .iter()
        .find(|c| matches!(c.origin, ProxyOrigin::FullProxy))
    {
        ordered.push(full);
    }

    if ordered.is_empty() {
        return None;
    }

    let has_successful_metadata = candidates
        .iter()
        .any(|candidate| candidate.artifact_status.as_deref() == Some("success"));

    if has_successful_metadata {
        ordered
            .into_iter()
            .find(|c| c.artifact_status.as_deref() == Some("success"))
    } else {
        ordered.into_iter().next()
    }
}

fn validate_proxy_candidate(candidate: &ProxyCandidate) -> Result<SelectedInput> {
    let probe = probe_video(&candidate.path)?;
    if probe.codec_name.as_deref() != Some("h264") {
        return Err(RendererError::Validation(format!(
            "Proxy at {} is not h264",
            candidate.path.display()
        )));
    }
    if probe.pix_fmt.as_deref() != Some("yuv420p") {
        return Err(RendererError::Validation(format!(
            "Proxy at {} has unexpected pixel format {:?}",
            candidate.path.display(),
            probe.pix_fmt
        )));
    }
    if let Some(has_b) = probe.has_b_frames {
        if has_b != 0 {
            return Err(RendererError::Validation(format!(
                "Proxy at {} contains B-frames",
                candidate.path.display()
            )));
        }
    }

    if let Some(fps) = probe.fps {
        if (fps - PROXY_EXPECTED_FPS as f64).abs() > 0.5 {
            return Err(RendererError::Validation(format!(
                "Proxy at {} has unexpected frame rate {:.2}",
                candidate.path.display(),
                fps
            )));
        }
    }

    Ok(SelectedInput {
        path: candidate.path.clone(),
        width: probe.width,
        height: probe.height,
        fps: probe.fps,
        origin: candidate.origin.clone(),
    })
}

fn proxy_label(origin: ProxyOrigin) -> &'static str {
    match origin {
        ProxyOrigin::PrimaryProxy => "render_proxy_1080p",
        ProxyOrigin::FullProxy => "render_proxy_full",
        ProxyOrigin::Original => "source",
    }
}

#[derive(Debug)]
struct ProxyCandidate {
    origin: ProxyOrigin,
    artifact_status: Option<String>,
    path: PathBuf,
}

fn find_artifact<'a>(
    metadata: Option<&'a RenderProxiesMetadata>,
    name: &str,
) -> Option<&'a RenderProxyArtifact> {
    metadata.and_then(|m| m.artifacts.iter().find(|a| a.name == name))
}

fn resolve_artifact_path(recording_dir: &Path, artifact_path: &str) -> PathBuf {
    let path = PathBuf::from(artifact_path);
    if path.is_absolute() {
        path
    } else {
        recording_dir.join(path)
    }
}

#[derive(Debug, Deserialize)]
struct FfprobeStreams {
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_name: Option<String>,
    codec_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    pix_fmt: Option<String>,
    avg_frame_rate: Option<String>,
    has_b_frames: Option<u32>,
}

#[derive(Debug)]
struct ProbeResult {
    width: u32,
    height: u32,
    fps: Option<f64>,
    codec_name: Option<String>,
    pix_fmt: Option<String>,
    has_b_frames: Option<u32>,
}

fn probe_video(path: &Path) -> Result<ProbeResult> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,codec_type,width,height,pix_fmt,avg_frame_rate,has_b_frames",
            "-print_format",
            "json",
        ])
        .arg(path)
        .output()?;

    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(RendererError::Ffprobe {
            path: path.to_path_buf(),
            message,
        });
    }

    let parsed: FfprobeStreams = serde_json::from_slice(&output.stdout)?;
    let stream = parsed
        .streams
        .into_iter()
        .find(|s| s.codec_type.as_deref() == Some("video"))
        .ok_or_else(|| RendererError::Validation("ffprobe returned no video stream".into()))?;

    let width = stream
        .width
        .ok_or_else(|| RendererError::Validation("ffprobe missing width".into()))?;
    let height = stream
        .height
        .ok_or_else(|| RendererError::Validation("ffprobe missing height".into()))?;
    let fps = stream.avg_frame_rate.as_deref().and_then(parse_frame_rate);

    Ok(ProbeResult {
        width,
        height,
        fps,
        codec_name: stream.codec_name,
        pix_fmt: stream.pix_fmt,
        has_b_frames: stream.has_b_frames,
    })
}

fn parse_frame_rate(value: &str) -> Option<f64> {
    if value.contains('/') {
        let mut parts = value.split('/');
        let num: f64 = parts.next()?.parse().ok()?;
        let den: f64 = parts.next()?.parse().ok()?;
        if den.abs() < f64::EPSILON {
            return None;
        }
        Some(num / den)
    } else {
        value.parse::<f64>().ok()
    }
}
