use std::path::{Path, PathBuf};
use std::time::Instant;

use ffmpeg::format;
use ffmpeg::media;
use ffmpeg_next as ffmpeg;
use serde::Serialize;

use crate::error::{RendererError, Result};
use crate::util::resolution::compute_target_dimensions;
use crate::video::{ensure_ffmpeg_initialized, VideoDecoder};

#[derive(Debug, Clone)]
pub struct ProxyRandomAccessBenchConfig {
    pub input_path: PathBuf,
    pub output_width: Option<u32>,
    pub samples: usize,
    pub warmup: usize,
    pub seed: u64,
}

#[derive(Debug, Serialize)]
pub struct ProxyRandomAccessBenchResult {
    pub input_path: String,
    pub input_width: u32,
    pub input_height: u32,
    pub duration_ms: f64,
    pub output_width: u32,
    pub output_height: u32,
    pub samples: usize,
    pub warmup: usize,
    pub seed: u64,
    pub seeks_per_second: f64,
    pub total_elapsed_ms: f64,
    pub stats_ms: SummaryStats,
}

#[derive(Debug, Serialize)]
pub struct SummaryStats {
    pub min: f64,
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
    pub mean: f64,
}

pub fn bench_proxy_random_access(
    config: ProxyRandomAccessBenchConfig,
) -> Result<ProxyRandomAccessBenchResult> {
    ensure_ffmpeg_initialized()?;

    if config.samples == 0 {
        return Err(RendererError::InvalidArgument("samples must be > 0".into()));
    }

    let (input_width, input_height, duration_ms) = probe_video_metadata(&config.input_path)?;
    if !(duration_ms.is_finite() && duration_ms > 0.0) {
        return Err(RendererError::Validation(
            "Video duration is not available".into(),
        ));
    }

    let (output_width, output_height) =
        compute_target_dimensions(input_width, input_height, config.output_width);

    let mut rng = XorShift64::new(config.seed);
    let max_timestamp_ms = (duration_ms - 1.0).max(0.0);

    let mut decoder = VideoDecoder::open(&config.input_path, output_width, output_height)?;
    let mut yuv_frame = ffmpeg::frame::Video::empty();
    let mut decoded_frame = ffmpeg::frame::Video::empty();

    for _ in 0..config.warmup {
        let t = rng.next_f64() * max_timestamp_ms;
        let _ = decoder.decode_yuv420p_frame_at_time_ms(t, &mut yuv_frame, &mut decoded_frame)?;
    }

    let mut samples_ms: Vec<f64> = Vec::with_capacity(config.samples);
    let bench_start = Instant::now();
    for _ in 0..config.samples {
        let t = rng.next_f64() * max_timestamp_ms;
        let t0 = Instant::now();
        let decoded =
            decoder.decode_yuv420p_frame_at_time_ms(t, &mut yuv_frame, &mut decoded_frame)?;
        if decoded.is_none() {
            return Err(RendererError::Validation(format!(
                "Failed to decode frame at timestamp {:.3}ms",
                t
            )));
        }
        samples_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
    }

    let total_elapsed_ms = bench_start.elapsed().as_secs_f64() * 1000.0;
    let seeks_per_second = (config.samples as f64) / (total_elapsed_ms / 1000.0).max(1e-9);

    let stats_ms = summarize_ms(&mut samples_ms)?;

    Ok(ProxyRandomAccessBenchResult {
        input_path: config.input_path.display().to_string(),
        input_width,
        input_height,
        duration_ms,
        output_width,
        output_height,
        samples: config.samples,
        warmup: config.warmup,
        seed: config.seed,
        seeks_per_second,
        total_elapsed_ms,
        stats_ms,
    })
}

fn probe_video_metadata(path: &Path) -> Result<(u32, u32, f64)> {
    let input = format::input(path)?;
    let stream = input
        .streams()
        .best(media::Type::Video)
        .ok_or_else(|| RendererError::Validation("No video stream found".into()))?;

    let decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?
        .decoder()
        .video()?;

    let width = decoder.width();
    let height = decoder.height();

    let duration_us = input.duration();
    let duration_ms = if duration_us > 0 {
        (duration_us as f64) / 1000.0
    } else {
        let stream_duration = stream.duration();
        if stream_duration > 0 {
            let tb = stream.time_base();
            let seconds = (stream_duration as f64)
                * (f64::from(tb.numerator()) / f64::from(tb.denominator().max(1)));
            seconds * 1000.0
        } else {
            0.0
        }
    };

    Ok((width, height, duration_ms))
}

fn summarize_ms(samples_ms: &mut [f64]) -> Result<SummaryStats> {
    if samples_ms.is_empty() {
        return Err(RendererError::InvalidArgument(
            "No samples to summarize".into(),
        ));
    }

    samples_ms.sort_by(|a, b| a.total_cmp(b));
    let min = samples_ms[0];
    let max = samples_ms[samples_ms.len() - 1];
    let mean = samples_ms.iter().copied().sum::<f64>() / (samples_ms.len() as f64);
    let p50 = percentile(samples_ms, 50.0);
    let p95 = percentile(samples_ms, 95.0);

    Ok(SummaryStats {
        min,
        p50,
        p95,
        max,
        mean,
    })
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.len() == 1 {
        return sorted[0];
    }
    let clamped = p.clamp(0.0, 100.0) / 100.0;
    let pos = clamped * (sorted.len() - 1) as f64;
    let idx = pos.floor() as usize;
    let frac = pos - (idx as f64);
    if idx + 1 >= sorted.len() {
        sorted[sorted.len() - 1]
    } else {
        sorted[idx] * (1.0 - frac) + sorted[idx + 1] * frac
    }
}

struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x1234_5678_9abc_def0
            } else {
                seed
            },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_f64(&mut self) -> f64 {
        let v = self.next_u64();
        let mantissa = v >> 11;
        (mantissa as f64) / ((1u64 << 53) as f64)
    }
}
