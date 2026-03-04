use rayon::prelude::*;

use crate::compositor::effects::{cursor, keystrokes, lens_warp, motion_blur, zoom};
use crate::compositor::i420_frame::{sample_bilinear_u8, I420Frame};
use crate::error::{RendererError, Result};
use crate::plan::types::ZoomWindow;

pub struct CpuCompositor {
    width: u32,
    height: u32,
    cursor_sprite: cursor::CursorSprite,
    keystroke_renderer: Option<keystrokes::KeystrokeRenderer>,
    motion_blur_config: motion_blur::MotionBlurConfig,
    scratch_zoomed: I420Frame,
    scratch_warped: I420Frame,
    scratch_blurred: I420Frame,
    scratch_output: I420Frame,
}

impl CpuCompositor {
    pub fn new(
        width: u32,
        height: u32,
        enable_keystrokes: bool,
        motion_blur_config: motion_blur::MotionBlurConfig,
    ) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(RendererError::InvalidArgument(
                "CpuCompositor requires non-zero dimensions".into(),
            ));
        }
        if width % 2 != 0 || height % 2 != 0 {
            return Err(RendererError::InvalidArgument(
                "CpuCompositor requires even dimensions for yuv420p".into(),
            ));
        }

        let cursor_sprite = cursor::CursorSprite::new();
        let keystroke_renderer = if enable_keystrokes {
            Some(keystrokes::KeystrokeRenderer::new(width, height)?)
        } else {
            None
        };

        Ok(Self {
            width,
            height,
            cursor_sprite,
            keystroke_renderer,
            motion_blur_config,
            scratch_zoomed: I420Frame::new(width, height)?,
            scratch_warped: I420Frame::new(width, height)?,
            scratch_blurred: I420Frame::new(width, height)?,
            scratch_output: I420Frame::new(width, height)?,
        })
    }

    pub fn swap_output_buffer(&mut self, buffer: &mut Vec<u8>) -> Result<()> {
        let expected = I420Frame::expected_len(self.width, self.height)?;
        if buffer.len() != expected || self.scratch_output.data.len() != expected {
            return Err(RendererError::InvalidArgument(format!(
                "swap_output_buffer buffer has wrong length (expected {expected}, got {})",
                buffer.len()
            )));
        }
        std::mem::swap(buffer, &mut self.scratch_output.data);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_frame<'a>(
        &mut self,
        input: &I420Frame,
        zoom_windows: &[ZoomWindow],
        output_time_ms: f64,
        frame_duration_ms: f64,
        cursor_state: Option<cursor::CursorState>,
        prev_cursor_state: Option<cursor::CursorState>,
        keystroke_state: Option<keystrokes::KeystrokeState<'a>>,
    ) -> Result<&I420Frame> {
        if input.width != self.width || input.height != self.height {
            return Err(RendererError::Validation(format!(
                "Frame dimension mismatch (expected {}x{}, got {}x{})",
                self.width, self.height, input.width, input.height
            )));
        }

        let zoom_state =
            zoom::compute_zoom_state(zoom_windows, output_time_ms, self.width, self.height);
        let prev_zoom_state = zoom::compute_zoom_state(
            zoom_windows,
            (output_time_ms - frame_duration_ms).max(0.0),
            self.width,
            self.height,
        );

        zoom::apply_zoom_pan_i420_into(input, zoom_state, &mut self.scratch_zoomed)?;

        let warp_params =
            lens_warp::compute_lens_warp_params(zoom_state.scale, zoom_state.focal_point);
        let warped = if let Some(params) = warp_params {
            apply_lens_warp_i420_into(&self.scratch_zoomed, params, &mut self.scratch_warped)?;
            true
        } else {
            false
        };

        let scene_in = if warped {
            &self.scratch_warped
        } else {
            &self.scratch_zoomed
        };

        let motion_y = motion_blur::compute_camera_motion_state(
            self.width,
            self.height,
            zoom_state,
            prev_zoom_state,
            self.motion_blur_config,
        );

        enum OutKind {
            Zoomed,
            Warped,
            Blurred,
        }

        // We check threshold here just to decide "Kind", but use a very small epsilon for quality
        let out_kind = if motion_y.estimated_max_blur_len_px > 0.001 {
            // 1. Blur Y Plane
            motion_blur::apply_camera_motion_blur_plane_into(
                scene_in.y_plane(),
                self.scratch_blurred.y_plane_mut(),
                self.width,
                self.height,
                motion_y,
            )?;

            // 2. Blur U/V Planes (CRITICAL FIX: Apply same motion blur to Chroma)
            // Need to scale the motion state for half-resolution U/V planes
            let uv_zoom_state = zoom::ZoomState {
                scale: zoom_state.scale,
                translate_x: zoom_state.translate_x * 0.5,
                translate_y: zoom_state.translate_y * 0.5,
                focal_point: zoom_state.focal_point,
            };
            let uv_prev_zoom_state = zoom::ZoomState {
                scale: prev_zoom_state.scale,
                translate_x: prev_zoom_state.translate_x * 0.5,
                translate_y: prev_zoom_state.translate_y * 0.5,
                focal_point: prev_zoom_state.focal_point,
            };

            let uv_width = self.width / 2;
            let uv_height = self.height / 2;

            let motion_uv = motion_blur::compute_camera_motion_state(
                uv_width,
                uv_height,
                uv_zoom_state,
                uv_prev_zoom_state,
                self.motion_blur_config,
            );

            motion_blur::apply_camera_motion_blur_plane_into(
                scene_in.u_plane(),
                self.scratch_blurred.u_plane_mut(),
                uv_width,
                uv_height,
                motion_uv,
            )?;

            motion_blur::apply_camera_motion_blur_plane_into(
                scene_in.v_plane(),
                self.scratch_blurred.v_plane_mut(),
                uv_width,
                uv_height,
                motion_uv,
            )?;

            OutKind::Blurred
        } else if warped {
            OutKind::Warped
        } else {
            OutKind::Zoomed
        };

        {
            let out = match out_kind {
                OutKind::Zoomed => &mut self.scratch_zoomed,
                OutKind::Warped => &mut self.scratch_warped,
                OutKind::Blurred => &mut self.scratch_blurred,
            };

            if let Some(cursor_state) = cursor_state {
                cursor::overlay_cursor_with_motion_blur_y_plane(
                    out.y_plane_mut(),
                    self.width,
                    self.height,
                    &self.cursor_sprite,
                    cursor_state,
                    prev_cursor_state,
                    zoom_state,
                    prev_zoom_state,
                    self.motion_blur_config.shutter_fraction(),
                    self.motion_blur_config.cursor_blur_reduction,
                    self.motion_blur_config.velocity_threshold,
                );
                // Borrow U/V planes in one shot to satisfy the borrow checker.
                let y_len = (self.width as usize).saturating_mul(self.height as usize);
                let uv_len = (self.width as usize / 2).saturating_mul(self.height as usize / 2);
                if out.data.len() >= y_len.saturating_add(uv_len.saturating_mul(2)) {
                    let uv = &mut out.data[y_len..];
                    let (u_plane, v_plane) = uv.split_at_mut(uv_len);
                    cursor::overlay_cursor_uv_planes(
                        u_plane,
                        v_plane,
                        self.width,
                        self.height,
                        &self.cursor_sprite,
                        cursor_state,
                        zoom_state,
                        1.0,
                    );
                }
            }

            if let Some(ks) = keystroke_state {
                if let Some(renderer) = self.keystroke_renderer.as_mut() {
                    renderer.overlay_y_plane(out.y_plane_mut(), ks)?;
                } else {
                    return Err(RendererError::Validation(
                        "Keystroke overlay requested but renderer is disabled".into(),
                    ));
                }
            }
        }

        match out_kind {
            OutKind::Zoomed => {
                std::mem::swap(&mut self.scratch_zoomed.data, &mut self.scratch_output.data)
            }
            OutKind::Warped => {
                std::mem::swap(&mut self.scratch_warped.data, &mut self.scratch_output.data)
            }
            OutKind::Blurred => std::mem::swap(
                &mut self.scratch_blurred.data,
                &mut self.scratch_output.data,
            ),
        }

        Ok(&self.scratch_output)
    }
}

fn apply_lens_warp_i420_into(
    input: &I420Frame,
    params: lens_warp::LensWarpParams,
    out: &mut I420Frame,
) -> Result<()> {
    if out.width != input.width || out.height != input.height {
        return Err(RendererError::InvalidArgument(
            "apply_lens_warp_i420_into output frame has wrong dimensions".into(),
        ));
    }
    let expected = I420Frame::expected_len(input.width, input.height)?;
    if input.data.len() != expected || out.data.len() != expected {
        return Err(RendererError::InvalidArgument(
            "apply_lens_warp_i420_into frame buffer has wrong length".into(),
        ));
    }

    apply_lens_warp_plane_into(
        input.y_plane(),
        out.y_plane_mut(),
        input.width,
        input.height,
        params,
        25,
        true,
    );
    let uv_width = input.uv_width();
    let uv_height = input.uv_height();
    apply_lens_warp_plane_into(
        input.u_plane(),
        out.u_plane_mut(),
        uv_width,
        uv_height,
        params,
        128,
        true,
    );
    apply_lens_warp_plane_into(
        input.v_plane(),
        out.v_plane_mut(),
        uv_width,
        uv_height,
        params,
        128,
        true,
    );
    Ok(())
}

fn apply_lens_warp_plane_into(
    input: &[u8],
    out: &mut [u8],
    width: u32,
    height: u32,
    params: lens_warp::LensWarpParams,
    background: u8,
    parallel: bool,
) {
    if width == 0 || height == 0 {
        return;
    }
    let row_len = width as usize;
    let expected = row_len.saturating_mul(height as usize);
    if input.len() != expected || out.len() != expected {
        return;
    }

    let w = width as f64;
    let h = height as f64;
    let max_x = (w - 1.0).max(0.0);
    let max_y = (h - 1.0).max(0.0);
    let inv_w = 1.0 / w.max(1.0);
    let inv_h = 1.0 / h.max(1.0);

    let (fx, fy) = params.focal_point;
    let sin_rx = params.rotate_x_deg.to_radians().sin();
    let sin_ry = params.rotate_y_deg.to_radians().sin();
    let perspective = params.perspective.max(1.0);
    let focal_x_px = fx * w;
    let focal_y_px = fy * h;

    if parallel {
        out.par_chunks_mut(row_len)
            .enumerate()
            .for_each(|(y, row)| {
                let y_px = y as f64 + 0.5;
                let cy = y_px - focal_y_px;
                let mut cx = 0.5 - focal_x_px;
                for x in 0..row_len {
                    let z_offset = cx * sin_ry + cy * sin_rx;
                    let denom = 1.0 + z_offset / perspective;
                    row[x] = if denom.abs() < 1e-9 {
                        background
                    } else {
                        let scale = 1.0 / denom;
                        let warped_u = fx + (cx * scale) * inv_w;
                        let warped_v = fy + (cy * scale) * inv_h;
                        if !(0.0..=1.0).contains(&warped_u) || !(0.0..=1.0).contains(&warped_v) {
                            background
                        } else {
                            let sx = (warped_u * max_x).clamp(0.0, max_x);
                            let sy = (warped_v * max_y).clamp(0.0, max_y);
                            sample_bilinear_u8(input, width, height, sx, sy)
                        }
                    };
                    cx += 1.0;
                }
            });
    } else {
        for (y, row) in out.chunks_mut(row_len).enumerate() {
            let y_px = y as f64 + 0.5;
            let cy = y_px - focal_y_px;
            let mut cx = 0.5 - focal_x_px;
            for x in 0..row_len {
                let z_offset = cx * sin_ry + cy * sin_rx;
                let denom = 1.0 + z_offset / perspective;
                row[x] = if denom.abs() < 1e-9 {
                    background
                } else {
                    let scale = 1.0 / denom;
                    let warped_u = fx + (cx * scale) * inv_w;
                    let warped_v = fy + (cy * scale) * inv_h;
                    if !(0.0..=1.0).contains(&warped_u) || !(0.0..=1.0).contains(&warped_v) {
                        background
                    } else {
                        let sx = (warped_u * max_x).clamp(0.0, max_x);
                        let sy = (warped_v * max_y).clamp(0.0, max_y);
                        sample_bilinear_u8(input, width, height, sx, sy)
                    }
                };
                cx += 1.0;
            }
        }
    }
}
