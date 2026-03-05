use crate::compositor::i420_frame::{sample_bilinear_u8, I420Frame};
use crate::easing::presets::{pan_ease, zoom_in_ease, zoom_out_ease};
use crate::error::{RendererError, Result};
use crate::plan::types::{ZoomFocusPoint, ZoomWindow};
use rayon::prelude::*;

pub const ZOOM_IN_DURATION_MS: f64 = 700.0;
pub const ZOOM_OUT_DURATION_MS: f64 = 700.0;
pub const PAN_DURATION_MS: f64 = 700.0;

#[derive(Debug, Clone, Copy)]
pub struct ZoomState {
    pub scale: f64,
    pub translate_x: f64,
    pub translate_y: f64,
    pub focal_point: (f64, f64),
}

pub fn compute_zoom_state(
    windows: &[ZoomWindow],
    time_ms: f64,
    video_width: u32,
    video_height: u32,
) -> ZoomState {
    debug_assert!(windows.windows(2).all(|w| w[0].start_ms <= w[1].start_ms));

    let Some((zoom_level, focus_x, focus_y)) = zoom_level_and_focus(windows, time_ms) else {
        return ZoomState {
            scale: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
            focal_point: (0.5, 0.5),
        };
    };

    if zoom_level <= 1.02 {
        return ZoomState {
            scale: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
            focal_point: (0.5, 0.5),
        };
    }
    let width = video_width as f64;
    let height = video_height as f64;
    let center_ratio_x = if width > 0.0 { focus_x / width } else { 0.5 };
    let center_ratio_y = if height > 0.0 { focus_y / height } else { 0.5 };

    let mut translate_x = (0.5 - center_ratio_x) * (zoom_level - 1.0) * width;
    let mut translate_y = (0.5 - center_ratio_y) * (zoom_level - 1.0) * height;

    translate_x = clamp_translation(translate_x, zoom_level, width);
    translate_y = clamp_translation(translate_y, zoom_level, height);

    ZoomState {
        scale: zoom_level,
        translate_x,
        translate_y,
        focal_point: (center_ratio_x, center_ratio_y),
    }
}

pub fn apply_zoom_pan_i420_into(
    input: &I420Frame,
    state: ZoomState,
    out: &mut I420Frame,
) -> Result<()> {
    if out.width != input.width || out.height != input.height {
        return Err(RendererError::InvalidArgument(
            "apply_zoom_pan_i420_into output frame has wrong dimensions".into(),
        ));
    }
    let expected_len = I420Frame::expected_len(input.width, input.height)?;
    if input.data.len() != expected_len || out.data.len() != expected_len {
        return Err(RendererError::InvalidArgument(
            "apply_zoom_pan_i420_into frame buffer has wrong length".into(),
        ));
    }

    if state.scale <= 1.00001
        && state.translate_x.abs() <= 0.00001
        && state.translate_y.abs() <= 0.00001
    {
        out.data.copy_from_slice(&input.data);
        return Ok(());
    }

    apply_zoom_pan_plane_into(
        input.y_plane(),
        input.width,
        input.height,
        state,
        out.y_plane_mut(),
    );

    let uv_state = ZoomState {
        scale: state.scale,
        translate_x: state.translate_x * 0.5,
        translate_y: state.translate_y * 0.5,
        focal_point: state.focal_point,
    };
    apply_zoom_pan_plane_into(
        input.u_plane(),
        input.uv_width(),
        input.uv_height(),
        uv_state,
        out.u_plane_mut(),
    );
    apply_zoom_pan_plane_into(
        input.v_plane(),
        input.uv_width(),
        input.uv_height(),
        uv_state,
        out.v_plane_mut(),
    );

    Ok(())
}

fn apply_zoom_pan_plane_into(
    input: &[u8],
    width: u32,
    height: u32,
    state: ZoomState,
    out: &mut [u8],
) {
    if width == 0 || height == 0 {
        return;
    }

    let row_len = width as usize;
    if out.len() != row_len.saturating_mul(height as usize) {
        return;
    }

    if state.scale <= 1.00001
        && state.translate_x.abs() <= 0.00001
        && state.translate_y.abs() <= 0.00001
    {
        out.copy_from_slice(input);
        return;
    }

    let width_f = width as f64;
    let height_f = height as f64;
    let cx = width_f / 2.0;
    let cy = height_f / 2.0;
    let inv_scale = 1.0 / state.scale.max(1e-9);
    let sx_start = cx + (0.0 - cx - state.translate_x) * inv_scale;
    let sx_step = inv_scale;

    out.par_chunks_mut(row_len)
        .enumerate()
        .for_each(|(y, out_row)| {
            let y_f = y as f64;
            let sy = cy + (y_f - cy - state.translate_y) * inv_scale;
            let mut sx = sx_start;
            for x in 0..row_len {
                out_row[x] = sample_bilinear_u8(input, width, height, sx, sy);
                sx += sx_step;
            }
        });
}

fn zoom_level_and_focus(windows: &[ZoomWindow], time_ms: f64) -> Option<(f64, f64, f64)> {
    for (index, window) in windows.iter().enumerate() {
        let next = windows.get(index + 1);
        let prev = if index > 0 {
            windows.get(index - 1)
        } else {
            None
        };

        let window_start = window.start_ms;
        let window_end = window.end_ms;

        let zoom_in_start = window_start - ZOOM_IN_DURATION_MS;
        if time_ms >= zoom_in_start && time_ms < window_start {
            let first = window.focus_points.first()?;
            let progress = (time_ms - zoom_in_start) / ZOOM_IN_DURATION_MS;
            let eased = zoom_in_ease(progress.clamp(0.0, 1.0));

            let mut start_zoom = 1.0;
            if let Some(prev_window) = prev {
                let prev_end = prev_window.end_ms;
                let prev_zoom_out_end = prev_end + ZOOM_OUT_DURATION_MS;
                if time_ms < prev_zoom_out_end {
                    let prev_progress = (time_ms - prev_end) / ZOOM_OUT_DURATION_MS;
                    let prev_eased = zoom_out_ease(prev_progress.clamp(0.0, 1.0));
                    start_zoom =
                        prev_window.zoom_level - (prev_window.zoom_level - 1.0) * prev_eased;
                }
            }

            return Some((
                start_zoom + (window.zoom_level - start_zoom) * eased,
                first.x,
                first.y,
            ));
        }

        if time_ms >= window_start && time_ms <= window_end {
            let focus = focus_point_at_time(&window.focus_points, time_ms);
            return Some((window.zoom_level, focus.0, focus.1));
        }

        let zoom_out_end = window_end + ZOOM_OUT_DURATION_MS;
        if time_ms > window_end && time_ms <= zoom_out_end {
            if let Some(next_window) = next {
                let next_zoom_in_start = next_window.start_ms - ZOOM_IN_DURATION_MS;
                if time_ms >= next_zoom_in_start {
                    continue;
                }
            }

            let last = window.focus_points.last()?;
            let progress = (time_ms - window_end) / ZOOM_OUT_DURATION_MS;
            let eased = zoom_out_ease(progress.clamp(0.0, 1.0));
            return Some((
                window.zoom_level - (window.zoom_level - 1.0) * eased,
                last.x,
                last.y,
            ));
        }
    }

    None
}

fn focus_point_at_time(points: &[ZoomFocusPoint], time_ms: f64) -> (f64, f64) {
    if points.is_empty() {
        return (0.0, 0.0);
    }
    if points.len() == 1 {
        return (points[0].x, points[0].y);
    }

    if time_ms <= points[0].time_ms {
        return (points[0].x, points[0].y);
    }

    if let Some(last) = points.last() {
        if time_ms >= last.time_ms {
            return (last.x, last.y);
        }
    }

    for window in points.windows(2) {
        let curr = &window[0];
        let next = &window[1];
        let curr_t = curr.time_ms;
        let next_t = next.time_ms;

        if time_ms < curr_t || time_ms > next_t {
            continue;
        }

        let gap = next_t - curr_t;
        if gap <= 0.0 {
            return (curr.x, curr.y);
        }

        let pan_start = next_t - PAN_DURATION_MS.min(gap * 0.8);
        if time_ms < pan_start {
            return (curr.x, curr.y);
        }

        let pan_duration = next_t - pan_start;
        if pan_duration <= 0.0 {
            return (next.x, next.y);
        }

        let progress = ((time_ms - pan_start) / pan_duration).clamp(0.0, 1.0);
        let eased = pan_ease(progress);
        return (
            curr.x + (next.x - curr.x) * eased,
            curr.y + (next.y - curr.y) * eased,
        );
    }

    (points[0].x, points[0].y)
}

fn clamp_translation(translate: f64, scale: f64, dimension: f64) -> f64 {
    let max_translate = (dimension * (scale - 1.0)) / 2.0;
    translate.clamp(-max_translate, max_translate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window_single(time_ms: f64, x: f64, y: f64) -> ZoomWindow {
        ZoomWindow {
            start_ms: time_ms,
            end_ms: time_ms + 1000.0,
            focus_points: vec![ZoomFocusPoint { time_ms, x, y }],
            zoom_level: 1.4,
        }
    }

    #[test]
    fn zoom_state_defaults_without_windows() {
        let state = compute_zoom_state(&[], 0.0, 1920, 1080);
        assert_eq!(state.scale, 1.0);
        assert_eq!(state.translate_x, 0.0);
        assert_eq!(state.translate_y, 0.0);
    }

    #[test]
    fn zoom_state_zoom_in_progresses() {
        let windows = vec![window_single(1000.0, 960.0, 540.0)];
        let state = compute_zoom_state(&windows, 800.0, 1920, 1080);
        assert!(state.scale > 1.0);
        assert!(state.scale < 1.4);
    }

    #[test]
    fn apply_zoom_pan_i420_identity_is_exact_copy() {
        let mut input = I420Frame::new(8, 8).unwrap();
        for (i, b) in input.data.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
        let mut out = I420Frame::new(8, 8).unwrap();
        apply_zoom_pan_i420_into(
            &input,
            ZoomState {
                scale: 1.0,
                translate_x: 0.0,
                translate_y: 0.0,
                focal_point: (0.5, 0.5),
            },
            &mut out,
        )
        .unwrap();
        assert_eq!(out.data, input.data);
    }
}
