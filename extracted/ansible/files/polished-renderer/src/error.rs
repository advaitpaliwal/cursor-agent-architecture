use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RendererError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ffmpeg error: {0}")]
    Ffmpeg(#[from] ffmpeg_next::Error),
    #[error("ffprobe failed for {path}: {message}")]
    Ffprobe { path: PathBuf, message: String },
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, RendererError>;
