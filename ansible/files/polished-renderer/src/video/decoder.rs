use std::path::Path;

use ffmpeg::codec::threading;
use ffmpeg::format;
use ffmpeg::frame;
use ffmpeg::media;
use ffmpeg::software::scaling;
use ffmpeg::Rational;
use ffmpeg_next as ffmpeg;

use crate::error::{RendererError, Result};
use crate::video::DecodedFrameInfo;

pub struct VideoDecoder {
    input: format::context::Input,
    stream_index: usize,
    time_base: Rational,
    decoder: ffmpeg::decoder::Video,
    scaler: Option<scaling::context::Context>,
    eof_sent: bool,
}

impl VideoDecoder {
    pub fn open(path: &Path, target_width: u32, target_height: u32) -> Result<Self> {
        let input = format::input(path)?;
        let stream = input
            .streams()
            .best(media::Type::Video)
            .ok_or_else(|| RendererError::Validation("No video stream found".into()))?;
        let stream_index = stream.index();
        let time_base = stream.time_base();

        let mut context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
        context.set_threading(threading::Config {
            kind: threading::Type::Frame,
            count: decoder_thread_count(),
            ..Default::default()
        });
        let decoder = context.decoder().video()?;

        let scaler = if decoder.width() == target_width
            && decoder.height() == target_height
            && decoder.format() == ffmpeg::format::Pixel::YUV420P
        {
            None
        } else {
            let flags = if decoder.width() == target_width && decoder.height() == target_height {
                scaling::flag::Flags::FAST_BILINEAR
            } else {
                scaling::flag::Flags::BILINEAR
            };

            Some(scaling::context::Context::get(
                decoder.format(),
                decoder.width(),
                decoder.height(),
                ffmpeg::format::Pixel::YUV420P,
                target_width,
                target_height,
                flags,
            )?)
        };

        Ok(Self {
            input,
            stream_index,
            time_base,
            decoder,
            scaler,
            eof_sent: false,
        })
    }

    fn seek_to_time_us(&mut self, timestamp_us: i64) -> Result<()> {
        self.input.seek(timestamp_us.max(0), ..)?;
        self.decoder.flush();
        self.eof_sent = false;
        Ok(())
    }

    pub fn decode_yuv420p_frame_at_time_ms(
        &mut self,
        timestamp_ms: f64,
        yuv420p_out: &mut frame::Video,
        decoded_scratch: &mut frame::Video,
    ) -> Result<Option<DecodedFrameInfo>> {
        let target_timestamp_us = (timestamp_ms * 1000.0).round() as i64;
        self.seek_to_time_us(target_timestamp_us)?;
        loop {
            match self.next_yuv420p_frame(yuv420p_out, decoded_scratch)? {
                Some(info) => {
                    if let Some(timestamp_us) = info.timestamp_us {
                        if timestamp_us >= target_timestamp_us {
                            return Ok(Some(info));
                        }
                        continue;
                    }
                    return Ok(Some(info));
                }
                None => return Ok(None),
            }
        }
    }

    pub fn decode_yuv420p_frame_from_current_time_ms(
        &mut self,
        timestamp_ms: f64,
        yuv420p_out: &mut frame::Video,
        decoded_scratch: &mut frame::Video,
    ) -> Result<Option<DecodedFrameInfo>> {
        let target_timestamp_us = (timestamp_ms * 1000.0).round() as i64;
        loop {
            match self.next_yuv420p_frame(yuv420p_out, decoded_scratch)? {
                Some(info) => {
                    if let Some(timestamp_us) = info.timestamp_us {
                        if timestamp_us >= target_timestamp_us {
                            return Ok(Some(info));
                        }
                        continue;
                    }
                    return Ok(Some(info));
                }
                None => return Ok(None),
            }
        }
    }

    pub fn next_yuv420p_frame(
        &mut self,
        yuv420p_out: &mut frame::Video,
        decoded_scratch: &mut frame::Video,
    ) -> Result<Option<DecodedFrameInfo>> {
        loop {
            if let Some(scaler) = self.scaler.as_mut() {
                match self.decoder.receive_frame(decoded_scratch) {
                    Ok(()) => {
                        scaler.run(decoded_scratch, yuv420p_out)?;
                        yuv420p_out.set_pts(decoded_scratch.pts());
                        let timestamp_us = decoded_scratch
                            .pts()
                            .map(|pts| pts_to_us(pts, self.time_base));
                        return Ok(Some(DecodedFrameInfo { timestamp_us }));
                    }
                    Err(ffmpeg::Error::Other { errno })
                        if errno == ffmpeg::util::error::EAGAIN
                            || errno == ffmpeg::util::error::EWOULDBLOCK => {}
                    Err(err) if err == ffmpeg::Error::Eof => return Ok(None),
                    Err(err) => return Err(err.into()),
                }
            } else {
                match self.decoder.receive_frame(yuv420p_out) {
                    Ok(()) => {
                        let timestamp_us =
                            yuv420p_out.pts().map(|pts| pts_to_us(pts, self.time_base));
                        return Ok(Some(DecodedFrameInfo { timestamp_us }));
                    }
                    Err(ffmpeg::Error::Other { errno })
                        if errno == ffmpeg::util::error::EAGAIN
                            || errno == ffmpeg::util::error::EWOULDBLOCK => {}
                    Err(err) if err == ffmpeg::Error::Eof => return Ok(None),
                    Err(err) => return Err(err.into()),
                }
            }

            if self.eof_sent {
                continue;
            }

            let mut packet_stream = None;
            for (stream, packet) in self.input.packets().take(1) {
                packet_stream = Some((stream, packet));
            }

            match packet_stream {
                Some((stream, packet)) => {
                    if stream.index() == self.stream_index {
                        self.decoder.send_packet(&packet)?;
                    }
                }
                None => {
                    self.decoder.send_eof()?;
                    self.eof_sent = true;
                }
            }
        }
    }
}

fn decoder_thread_count() -> usize {
    std::env::var("POLISHED_RENDERER_DECODER_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(1)
}

fn pts_to_us(pts: i64, time_base: Rational) -> i64 {
    let numerator = i64::from(time_base.numerator());
    let denominator = i64::from(time_base.denominator());
    if denominator == 0 {
        return 0;
    }
    pts.saturating_mul(numerator).saturating_mul(1_000_000) / denominator
}
