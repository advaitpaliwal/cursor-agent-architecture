use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::error::{RendererError, Result};

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    format: Option<FfprobeFormat>,
    streams: Option<Vec<FfprobeStream>>,
}

pub struct OutputVideoExpectations {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub frames: u32,
}

pub fn verify_output_video(path: &Path, expected: OutputVideoExpectations) -> Result<()> {
    if expected.width == 0 || expected.height == 0 || expected.frames == 0 {
        return Err(RendererError::InvalidArgument(
            "OutputVideoExpectations must be non-zero".into(),
        ));
    }
    if !(expected.fps.is_finite() && expected.fps > 0.0) {
        return Err(RendererError::InvalidArgument(
            "OutputVideoExpectations fps must be positive".into(),
        ));
    }

    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_type,width,height,avg_frame_rate",
            "-show_entries",
            "format=duration",
            "-print_format",
            "json",
        ])
        .arg(path)
        .output()?;

    if !output.status.success() {
        return Err(RendererError::Ffprobe {
            path: path.to_path_buf(),
            message: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    let parsed: FfprobeOutput = serde_json::from_slice(&output.stdout)?;
    let stream = parsed
        .streams
        .unwrap_or_default()
        .into_iter()
        .find(|s| s.codec_type.as_deref() == Some("video"))
        .ok_or_else(|| RendererError::Validation("ffprobe returned no video stream".into()))?;

    let width = stream
        .width
        .ok_or_else(|| RendererError::Validation("ffprobe missing width".into()))?;
    let height = stream
        .height
        .ok_or_else(|| RendererError::Validation("ffprobe missing height".into()))?;
    if width != expected.width || height != expected.height {
        return Err(RendererError::Validation(format!(
            "Output dimensions mismatch (expected {}x{}, got {}x{})",
            expected.width, expected.height, width, height
        )));
    }

    let fps = stream
        .avg_frame_rate
        .as_deref()
        .and_then(parse_frame_rate)
        .unwrap_or(expected.fps);
    if (fps - expected.fps).abs() > 0.5 {
        return Err(RendererError::Validation(format!(
            "Output fps mismatch (expected {:.2}, got {:.2})",
            expected.fps, fps
        )));
    }

    let duration_s = parsed
        .format
        .and_then(|f| f.duration)
        .and_then(|d| d.parse::<f64>().ok())
        .unwrap_or(0.0);
    if !(duration_s.is_finite() && duration_s > 0.0) {
        return Err(RendererError::Validation(
            "Output duration is missing or invalid".into(),
        ));
    }

    let expected_duration_s = (expected.frames as f64) / expected.fps;
    let min_duration_s = (expected_duration_s - 0.5).max(0.0);
    if duration_s < min_duration_s {
        return Err(RendererError::Validation(format!(
            "Output duration too short (expected ~{expected_duration_s:.2}s, got {duration_s:.2}s)"
        )));
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_frame_rate_supports_fraction() {
        assert_eq!(parse_frame_rate("60/1").unwrap(), 60.0);
    }

    #[test]
    fn expectations_reject_zero() {
        let err = verify_output_video(
            Path::new("does-not-matter.mp4"),
            OutputVideoExpectations {
                width: 0,
                height: 10,
                fps: 60.0,
                frames: 1,
            },
        )
        .unwrap_err();
        match err {
            RendererError::InvalidArgument(_) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
