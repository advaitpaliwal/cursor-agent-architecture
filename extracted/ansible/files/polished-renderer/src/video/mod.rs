mod decoder;
mod encoder;
mod verify;

#[derive(Debug, Clone, Copy)]
pub(crate) struct DecodedFrameInfo {
    pub timestamp_us: Option<i64>,
}

pub use decoder::VideoDecoder;
pub use encoder::VideoEncoder;
pub use verify::{verify_output_video, OutputVideoExpectations};

use crate::error::Result;

pub fn ensure_ffmpeg_initialized() -> Result<()> {
    use std::sync::OnceLock;

    static INIT_RESULT: OnceLock<std::result::Result<(), ffmpeg_next::Error>> = OnceLock::new();
    match *INIT_RESULT.get_or_init(ffmpeg_next::init) {
        Ok(()) => Ok(()),
        Err(err) => Err(err.into()),
    }
}
