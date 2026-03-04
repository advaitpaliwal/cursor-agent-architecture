use std::io::{BufWriter, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};

use tracing::info;

use crate::error::{RendererError, Result};

pub struct VideoEncoder {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    width: u32,
    height: u32,
}

impl VideoEncoder {
    pub fn new(
        output_path: &Path,
        width: u32,
        height: u32,
        fps: f64,
        realtime: bool,
    ) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(RendererError::InvalidArgument(
                "VideoEncoder requires non-zero dimensions".into(),
            ));
        }
        if width % 2 != 0 || height % 2 != 0 {
            return Err(RendererError::InvalidArgument(
                "VideoEncoder requires even dimensions for yuv420p".into(),
            ));
        }
        if !(fps.is_finite() && fps > 0.0) {
            return Err(RendererError::InvalidArgument(
                "VideoEncoder requires positive fps".into(),
            ));
        }

        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-y")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-f")
            .arg("rawvideo")
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg("-s")
            .arg(format!("{width}x{height}"))
            .arg("-r")
            .arg(format!("{fps}"))
            .arg("-i")
            .arg("pipe:0")
            .arg("-an")
            .arg("-c:v")
            .arg("libx264")
            // QuickTime compatibility / sane defaults:
            // - avoid B-frames (simpler decode + fewer "can't open" edge cases)
            // - force the common 'avc1' tag for H.264 in MP4/MOV
            .arg("-profile:v")
            .arg("high")
            .arg("-bf")
            .arg("0")
            .arg("-tag:v")
            .arg("avc1");

        if let Some(threads) = encoder_thread_count_from_env() {
            cmd.arg("-threads").arg(format!("{threads}"));
        }

        cmd.arg("-pix_fmt")
            .arg("yuv420p")
            .arg("-crf")
            .arg("20")
            .arg("-preset")
            .arg(if realtime { "ultrafast" } else { "veryfast" })
            .arg("-movflags")
            .arg("+faststart")
            .arg("-metadata")
            .arg("comment=Made with Cursor")
            .arg("-metadata")
            .arg("encoder=Cursor Polished Renderer")
            .arg(output_path);

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::inherit());

        info!("Spawning ffmpeg encoder -> {}", output_path.display());
        let mut child = cmd.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| RendererError::Other("Failed to open ffmpeg stdin".into()))?;

        Ok(Self {
            child,
            stdin: BufWriter::new(stdin),
            width,
            height,
        })
    }

    pub fn write_frame_yuv420p(&mut self, frame_yuv420p: &[u8]) -> Result<()> {
        let expected = (self.width as usize)
            .saturating_mul(self.height as usize)
            .saturating_mul(3)
            .saturating_div(2);
        if frame_yuv420p.len() != expected {
            return Err(RendererError::InvalidArgument(format!(
                "yuv420p frame buffer has wrong length (expected {expected}, got {})",
                frame_yuv420p.len()
            )));
        }
        self.stdin.write_all(frame_yuv420p)?;
        Ok(())
    }

    pub fn finish(mut self) -> Result<()> {
        self.stdin.flush()?;
        drop(self.stdin);
        let status = self.child.wait()?;
        if !status.success() {
            return Err(RendererError::Other(format!(
                "ffmpeg encoder failed with status {status}"
            )));
        }
        Ok(())
    }
}

fn encoder_thread_count_from_env() -> Option<usize> {
    std::env::var("POLISHED_RENDERER_ENCODER_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n > 0)
}
