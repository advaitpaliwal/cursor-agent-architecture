use crate::compositor::i420_frame::sample_bilinear_u8;
use crate::error::{RendererError, Result};

use super::zoom::ZoomState;
use clap::ValueEnum;
use rayon::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct MotionBlurConfig {
    pub shutter_angle: f64,
    pub max_blur_fraction: f64,
    pub cursor_blur_reduction: f64,
    pub velocity_threshold: f64,
    pub quality: MotionBlurQuality,
}

impl MotionBlurConfig {
    pub fn shutter_fraction(self) -> f64 {
        (self.shutter_angle / 360.0).clamp(0.0, 1.0)
    }
}

impl Default for MotionBlurConfig {
    fn default() -> Self {
        Self {
            shutter_angle: 360.0,
            max_blur_fraction: 1.0,
            cursor_blur_reduction: 0.4,
            velocity_threshold: 2.0,
            quality: MotionBlurQuality::High,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "lower")]
pub enum MotionBlurQuality {
    Low,
    Medium,
    High,
}

impl MotionBlurQuality {
    fn max_samples(self) -> usize {
        match self {
            MotionBlurQuality::Low => 8,
            MotionBlurQuality::Medium => 16,
            MotionBlurQuality::High => 32,
        }
    }
}

// Interleaved Gradient Noise (IGN)
// Returns a pseudo-random float [0.0, 1.0] based on screen coordinates.
// This specific magic number set is standard for high-frequency noise
// that effectively hides aliasing.
#[inline(always)]
fn gradient_noise(x: f64, y: f64) -> f64 {
    let f = 52.9829189 * x + 0.06711056 * y;
    (f.sin() * 43758.5453).fract()
}

#[derive(Debug, Clone, Copy)]
pub struct CameraMotionState {
    pub curr_zoom: ZoomState,
    pub prev_zoom: ZoomState,
    pub shutter_fraction: f64,
    pub max_blur_px: f64,
    pub velocity_threshold_px: f64,
    pub estimated_max_blur_len_px: f64,
    // Used as the upper limit for adaptive sampling
    pub max_sample_count: usize,
}

fn estimate_max_blur_len_px(
    width: u32,
    height: u32,
    curr_zoom: ZoomState,
    prev_zoom: ZoomState,
    shutter_fraction: f64,
) -> f64 {
    if width == 0 || height == 0 {
        return 0.0;
    }
    let w = width as f64;
    let h = height as f64;
    let corners = [
        (0.0, 0.0),
        (w - 1.0, 0.0),
        (0.0, h - 1.0),
        (w - 1.0, h - 1.0),
    ];

    let mut max_len = 0.0;
    for (x, y) in corners {
        let (mvx, mvy) = motion_vector_px_at(x, y, width, height, curr_zoom, prev_zoom);
        let len = ((mvx * shutter_fraction).powi(2) + (mvy * shutter_fraction).powi(2)).sqrt();
        if len > max_len {
            max_len = len;
        }
    }
    max_len
}

fn motion_vector_px_at(
    x: f64,
    y: f64,
    width: u32,
    height: u32,
    curr_zoom: ZoomState,
    prev_zoom: ZoomState,
) -> (f64, f64) {
    let cx = (width as f64) * 0.5;
    let cy = (height as f64) * 0.5;

    let curr_scale = curr_zoom.scale.max(1e-9);
    let sx = cx + (x - cx - curr_zoom.translate_x) / curr_scale;
    let sy = cy + (y - cy - curr_zoom.translate_y) / curr_scale;

    let prev_x = cx + (sx - cx) * prev_zoom.scale + prev_zoom.translate_x;
    let prev_y = cy + (sy - cy) * prev_zoom.scale + prev_zoom.translate_y;

    (x - prev_x, y - prev_y)
}

pub fn compute_camera_motion_state(
    width: u32,
    height: u32,
    curr_zoom: ZoomState,
    prev_zoom: ZoomState,
    config: MotionBlurConfig,
) -> CameraMotionState {
    let diag_px = (((width as f64).powi(2) + (height as f64).powi(2)).sqrt()).max(1.0);
    let shutter_fraction = config.shutter_fraction();
    let max_blur_px = (config.max_blur_fraction * diag_px).max(0.0);
    let velocity_threshold_px = config.velocity_threshold.max(0.0);

    let estimated_max_blur_len_px =
        estimate_max_blur_len_px(width, height, curr_zoom, prev_zoom, shutter_fraction);

    let max_sample_count = 128; // Highest quality overriding config

    CameraMotionState {
        curr_zoom,
        prev_zoom,
        shutter_fraction,
        max_blur_px,
        velocity_threshold_px,
        estimated_max_blur_len_px,
        max_sample_count,
    }
}

pub fn apply_camera_motion_blur_plane_into(
    input: &[u8],
    out: &mut [u8],
    width: u32,
    height: u32,
    motion: CameraMotionState,
) -> Result<bool> {
    if width == 0 || height == 0 {
        return Ok(false);
    }
    let row_len = width as usize;
    let expected = row_len.saturating_mul(height as usize);
    if input.len() != expected || out.len() != expected {
        return Err(RendererError::InvalidArgument(
            "apply_camera_motion_blur_plane_into plane buffer has wrong length".into(),
        ));
    }

    if motion.estimated_max_blur_len_px < 0.001 {
        out.copy_from_slice(input);
        return Ok(false);
    }

    let width_f = width as f64;
    let height_f = height as f64;
    let cx = width_f * 0.5;
    let cy = height_f * 0.5;

    let curr_scale = motion.curr_zoom.scale.max(1e-9);
    let curr_scale_inv = 1.0 / curr_scale;
    let prev_scale = motion.prev_zoom.scale;
    let shutter = motion.shutter_fraction;

    let sx_start = cx + (0.5 - cx - motion.curr_zoom.translate_x) * curr_scale_inv;
    let sx_step = curr_scale_inv;
    let prev_x_start = cx + (sx_start - cx) * prev_scale + motion.prev_zoom.translate_x;
    let prev_x_step = sx_step * prev_scale;

    let blur_x_start = (0.5 - prev_x_start) * shutter;
    let blur_x_step = (1.0 - prev_x_step) * shutter;

    // Target stride for adaptive sampling (pixels per sample)
    // 0.5 for supersampling quality
    let target_stride = 0.5;

    out.par_chunks_mut(row_len)
        .enumerate()
        .for_each(|(y, out_row)| {
            let in_row = &input[y * row_len..y * row_len + row_len];
            let y_f = y as f64 + 0.5;
            let base_y = y as f64;

            let sy = cy + (y_f - cy - motion.curr_zoom.translate_y) * curr_scale_inv;
            let prev_y = cy + (sy - cy) * prev_scale + motion.prev_zoom.translate_y;
            let blur_y = (y_f - prev_y) * shutter;

            let mut blur_x = blur_x_start;
            for x in 0..row_len {
                let mut bx = blur_x;
                let mut by = blur_y;

                let mut blur_len = (bx * bx + by * by).sqrt();
                if blur_len < 0.001 {
                    out_row[x] = in_row[x];
                    blur_x += blur_x_step;
                    continue;
                }

                if motion.max_blur_px > 0.0 && blur_len > motion.max_blur_px {
                    let scale = motion.max_blur_px / blur_len.max(1e-9);
                    bx *= scale;
                    by *= scale;
                    blur_len = motion.max_blur_px;
                }

                // Adaptive sampling: calculate sample count based on velocity
                let raw_samples = (blur_len / target_stride).ceil();
                // Clamp samples between 4 and max_sample_count (e.g. 32)
                let samples = (raw_samples as usize).clamp(4, motion.max_sample_count);

                // Jitter for stochastic sampling to trade aliasing for noise
                let jitter = gradient_noise(x as f64, y as f64);

                let inv_samples = 1.0 / (samples as f64);
                let inv_len = 1.0 / blur_len.max(1e-9);
                let dir_x = bx * inv_len;
                let dir_y = by * inv_len;

                let base_x = x as f64;
                let mut acc = 0.0f64;

                for i in 0..samples {
                    // t_normalized in [0, 1]
                    let t_normalized = (i as f64 + jitter) * inv_samples;
                    // t centered at 0 in [-0.5, 0.5]
                    let t = t_normalized - 0.5;

                    let ox = dir_x * blur_len * t;
                    let oy = dir_y * blur_len * t;
                    let sample = sample_bilinear_u8(input, width, height, base_x + ox, base_y + oy);
                    acc += sample as f64;
                }

                // Box filter weight is uniform (1.0 / samples)
                out_row[x] = (acc * inv_samples).round().clamp(0.0, 255.0) as u8;

                blur_x += blur_x_step;
            }
        });

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_skips_when_no_motion() {
        let width = 32u32;
        let height = 16u32;
        let input = vec![123u8; (width as usize) * (height as usize)];
        let mut out = vec![0u8; input.len()];
        let cfg = MotionBlurConfig::default();
        let motion = compute_camera_motion_state(
            width,
            height,
            ZoomState {
                scale: 1.0,
                translate_x: 0.0,
                translate_y: 0.0,
                focal_point: (0.5, 0.5),
            },
            ZoomState {
                scale: 1.0,
                translate_x: 0.0,
                translate_y: 0.0,
                focal_point: (0.5, 0.5),
            },
            cfg,
        );
        let applied =
            apply_camera_motion_blur_plane_into(&input, &mut out, width, height, motion).unwrap();
        assert!(!applied);
        assert_eq!(out, input);
    }

    #[test]
    fn blur_preserves_solid_color() {
        let width = 64u32;
        let height = 32u32;
        let input = vec![200u8; (width as usize) * (height as usize)];
        let mut out = vec![0u8; input.len()];
        let cfg = MotionBlurConfig {
            velocity_threshold: 0.0,
            ..MotionBlurConfig::default()
        };
        let motion = compute_camera_motion_state(
            width,
            height,
            ZoomState {
                scale: 1.0,
                translate_x: 10.0,
                translate_y: 0.0,
                focal_point: (0.5, 0.5),
            },
            ZoomState {
                scale: 1.0,
                translate_x: 0.0,
                translate_y: 0.0,
                focal_point: (0.5, 0.5),
            },
            cfg,
        );
        let _ =
            apply_camera_motion_blur_plane_into(&input, &mut out, width, height, motion).unwrap();

        // Since input is solid color, average should be equal to input (plus/minus rounding errors)
        // With dithering/noise, there might be slight variance, but for solid 200, it should be stable.
        // Actually, with edge clamping in sample_bilinear, and solid color, it should be exactly 200.
        // However, noise *could* affect float precision slightly.
        // Let's check a few pixels.
        assert_eq!(out[0], 200);
    }
}
