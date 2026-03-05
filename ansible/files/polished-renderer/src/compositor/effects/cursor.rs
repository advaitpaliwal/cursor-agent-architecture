use crate::compositor::effects::zoom::ZoomState;
use crate::compositor::frame::RgbaFrame;
use crate::easing::presets::screen_studio_cursor_ease;
use crate::plan::types::{ClickEffectKeyframe, ClickType, CursorPathKeyframe};
use rayon::prelude::*;

const CURSOR_SVG: &str = include_str!("../../../assets/cursor.svg");

pub const CURSOR_MOVE_MS: f64 = 600.0;
pub const BASE_CURSOR_SIZE_PX: f64 = 32.0;
pub const BASE_VIDEO_WIDTH_PX: f64 = 1920.0;

pub const HOTSPOT_RATIO_X: f64 = 3.0 / 24.0;
pub const HOTSPOT_RATIO_Y: f64 = 2.0 / 24.0;

const ICON_SCALE: f64 = 1.25;

const DEPRESS_ANTICIPATION_MS: f64 = 50.0;
const DEPRESS_MS: f64 = 80.0;
const RELEASE_MS: f64 = 150.0;
const DEPRESS_SCALE: f64 = 0.75;

#[derive(Debug, Clone, Copy)]
pub struct CursorState {
    pub x: f64,
    pub y: f64,
    pub depress_scale: f64,
}

#[derive(Clone)]
pub struct CursorSprite {
    base: RgbaFrame,
}

impl CursorSprite {
    pub fn new() -> Self {
        Self {
            base: load_cursor_sprite_from_png(),
        }
    }

    pub fn width(&self) -> u32 {
        self.base.width
    }

    pub fn height(&self) -> u32 {
        self.base.height
    }

    pub fn sample_bilinear(&self, x: f64, y: f64) -> [u8; 4] {
        self.base.sample_bilinear(x, y)
    }
}

/// Compute the cursor state at a given time in **output** timeline.
///
/// `keyframes` must be sorted ascending by `video_timestamp_ms`.
pub fn compute_cursor_state(
    time_ms: f64,
    keyframes: &[ClickEffectKeyframe],
) -> Option<CursorState> {
    let first = keyframes.first()?;
    let first_move_start = first.video_timestamp_ms - CURSOR_MOVE_MS;
    if time_ms < first_move_start {
        return Some(CursorState {
            x: first.x,
            y: first.y,
            depress_scale: 1.0,
        });
    }

    let mut current_idx = 0usize;
    for (idx, keyframe) in keyframes.iter().enumerate() {
        if time_ms >= keyframe.video_timestamp_ms {
            current_idx = idx;
        } else {
            break;
        }
    }

    let current = &keyframes[current_idx];
    let next = keyframes.get(current_idx + 1);

    let mut time_since_click_ms: Option<f64> = None;
    if is_actual_click(current.click_type) {
        let elapsed = time_ms - current.video_timestamp_ms;
        if elapsed < DEPRESS_MS + RELEASE_MS {
            time_since_click_ms = Some(elapsed);
        }
    }

    if let Some(next) = next {
        if is_actual_click(next.click_type) {
            let time_until_next = next.video_timestamp_ms - time_ms;
            if time_until_next > 0.0 && time_until_next <= DEPRESS_ANTICIPATION_MS {
                time_since_click_ms = Some(-time_until_next);
            }
        }
    }

    let mut x = current.x;
    let mut y = current.y;

    if let Some(next) = next {
        let gap = next.video_timestamp_ms - current.video_timestamp_ms;
        if gap > 0.0 {
            let distance = ((next.x - current.x).powi(2) + (next.y - current.y).powi(2)).sqrt();
            let distance_factor = (distance / 400.0).sqrt();
            let scaled_duration = CURSOR_MOVE_MS * distance_factor.min(1.5);
            let move_duration = scaled_duration.max(1.0).min(gap * 0.8);
            let move_start = next.video_timestamp_ms - move_duration;

            if time_ms >= move_start {
                let progress = ((time_ms - move_start) / move_duration).clamp(0.0, 1.0);
                let eased = screen_studio_cursor_ease(progress);
                x = current.x + (next.x - current.x) * eased;
                y = current.y + (next.y - current.y) * eased;
            }
        }
    }

    Some(CursorState {
        x,
        y,
        depress_scale: depress_scale(time_since_click_ms),
    })
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

pub fn overlay_cursor_with_motion_blur_y_plane(
    y_plane: &mut [u8],
    width: u32,
    height: u32,
    sprite: &CursorSprite,
    cursor: CursorState,
    prev_cursor: Option<CursorState>,
    curr_zoom: ZoomState,
    prev_zoom: ZoomState,
    shutter_fraction: f64,
    blur_reduction: f64,
    velocity_threshold_px: f64,
) {
    if width == 0 || height == 0 {
        return;
    }
    let expected = (width as usize).saturating_mul(height as usize);
    if y_plane.len() != expected {
        return;
    }

    let Some(prev_cursor) = prev_cursor else {
        overlay_cursor_impl_y_plane(
            y_plane,
            width,
            height,
            sprite,
            cursor,
            curr_zoom.scale,
            curr_zoom.translate_x,
            curr_zoom.translate_y,
            None,
            1.0,
        );
        return;
    };

    let (curr_tip_x, curr_tip_y) = cursor_tip_in_frame(
        width,
        height,
        cursor.x,
        cursor.y,
        curr_zoom.scale,
        curr_zoom.translate_x,
        curr_zoom.translate_y,
    );
    let (prev_tip_x, prev_tip_y) = cursor_tip_in_frame(
        width,
        height,
        prev_cursor.x,
        prev_cursor.y,
        prev_zoom.scale,
        prev_zoom.translate_x,
        prev_zoom.translate_y,
    );

    let vx = curr_tip_x - prev_tip_x;
    let vy = curr_tip_y - prev_tip_y;

    let shutter = shutter_fraction.clamp(0.0, 1.0);
    let reduction = blur_reduction.clamp(0.0, 1.0);
    let blur_x = vx * shutter * reduction;
    let blur_y = vy * shutter * reduction;
    let blur_len = (blur_x * blur_x + blur_y * blur_y).sqrt();

    if blur_len < velocity_threshold_px.max(0.0) {
        overlay_cursor_impl_y_plane(
            y_plane,
            width,
            height,
            sprite,
            cursor,
            curr_zoom.scale,
            curr_zoom.translate_x,
            curr_zoom.translate_y,
            Some((curr_tip_x, curr_tip_y)),
            1.0,
        );
        return;
    }

    // Adaptive sampling for "highest quality"
    // We aim for a stride of ~0.5 to 1.0 pixel to be super smooth.
    // Max samples 64 because "I don't care about performance".
    let target_stride = 0.5;
    let raw_samples = (blur_len / target_stride).ceil();
    let samples = (raw_samples as usize).clamp(8, 64);

    let inv_samples = 1.0 / (samples as f64);

    // Compute bounds for the gather loop
    // Cursor geometry
    let cursor_size = (BASE_CURSOR_SIZE_PX * (width as f64 / BASE_VIDEO_WIDTH_PX))
        .round()
        .max(2.0);
    let hotspot_x = HOTSPOT_RATIO_X * cursor_size;
    let hotspot_y = HOTSPOT_RATIO_Y * cursor_size;
    let icon_scale = (cursor.depress_scale * ICON_SCALE).max(0.01);
    let total_scale = curr_zoom.scale * icon_scale;

    if total_scale <= 0.0 {
        return;
    }

    let container_size = cursor_size * curr_zoom.scale;
    // Icon size in screen pixels
    let icon_size_px = cursor_size * icon_scale * curr_zoom.scale;

    // Sprite scaling factors
    let base_w = sprite.width() as f64;
    let base_h = sprite.height() as f64;
    let sprite_scale_x = icon_size_px / base_w.max(1.0);
    let sprite_scale_y = icon_size_px / base_h.max(1.0);

    // Bounding Box Calculation
    // We sweep the cursor tip from `curr - blur/2` to `curr + blur/2` (centered blur).
    // The cursor icon is offset from the tip by `hotspot`.
    // Icon box at a given tip position (tx, ty):
    //   cx = tx - hotspot_x * zoom + container_size/2
    //   cy = ty - hotspot_y * zoom + container_size/2
    //   x = cx - icon_size/2
    //   y = cy - icon_size/2
    // Simplify:
    //   icon_left = tx - (hotspot_x * zoom) + (container_size - icon_size)/2
    //   icon_top = ty - (hotspot_y * zoom) + (container_size - icon_size)/2

    // Let's compute the icon offset from the tip
    let offset_x = -(hotspot_x * curr_zoom.scale) + (container_size - icon_size_px) * 0.5;
    let offset_y = -(hotspot_y * curr_zoom.scale) + (container_size - icon_size_px) * 0.5;

    // The motion path of the tip center
    // t goes from -0.5 to 0.5
    // p(t) = curr_tip + blur * t
    let tip_start_x = curr_tip_x + blur_x * -0.5;
    let tip_start_y = curr_tip_y + blur_y * -0.5;
    let tip_end_x = curr_tip_x + blur_x * 0.5;
    let tip_end_y = curr_tip_y + blur_y * 0.5;

    // Union of start and end rects plus padding
    let min_tip_x = tip_start_x.min(tip_end_x);
    let max_tip_x = tip_start_x.max(tip_end_x);
    let min_tip_y = tip_start_y.min(tip_end_y);
    let max_tip_y = tip_start_y.max(tip_end_y);

    let bbox_x0 = (min_tip_x + offset_x).floor() as i64;
    let bbox_y0 = (min_tip_y + offset_y).floor() as i64;
    let bbox_x1 = (max_tip_x + offset_x + icon_size_px).ceil() as i64;
    let bbox_y1 = (max_tip_y + offset_y + icon_size_px).ceil() as i64;

    // Clamp to screen
    let start_y = bbox_y0.clamp(0, height as i64) as usize;
    let end_y = bbox_y1.clamp(0, height as i64) as usize;
    let start_x = bbox_x0.clamp(0, width as i64) as usize;
    let end_x = bbox_x1.clamp(0, width as i64) as usize;

    if start_x >= end_x || start_y >= end_y {
        return;
    }

    let row_len = width as usize;

    // Parallelize over rows for performance (even though "don't care", let's not hang the UI)
    y_plane
        .par_chunks_mut(row_len)
        .enumerate()
        .for_each(|(y, row)| {
            if y < start_y || y >= end_y {
                return;
            }

            let y_f = y as f64 + 0.5;

            for x in start_x..end_x {
                let x_f = x as f64 + 0.5;

                // Stochastic Jitter
                let jitter = gradient_noise(x_f, y_f);

                let mut acc_a = 0.0;
                let mut acc_luma = 0.0;
                let mut hits = 0;

                for i in 0..samples {
                    // t in [-0.5, 0.5]
                    let t_normalized = (i as f64 + jitter) * inv_samples;
                    let t = t_normalized - 0.5;

                    let tip_x = curr_tip_x + blur_x * t;
                    let tip_y = curr_tip_y + blur_y * t;

                    // Compute sprite coordinates
                    let icon_left = tip_x + offset_x;
                    let icon_top = tip_y + offset_y;

                    // Map screen pixel (x_f, y_f) to sprite UV
                    let sx = (x_f - icon_left) / sprite_scale_x.max(1e-9);
                    let sy = (y_f - icon_top) / sprite_scale_y.max(1e-9);

                    if sx >= 0.0 && sy >= 0.0 && sx < base_w && sy < base_h {
                        let src = sprite.sample_bilinear(sx, sy);
                        // src is [R, G, B, A]
                        if src[3] > 0 {
                            let a = src[3] as f64 / 255.0;
                            let luma = 0.2126 * (src[0] as f64)
                                + 0.7152 * (src[1] as f64)
                                + 0.0722 * (src[2] as f64);

                            // Premultiplied accumulation
                            acc_a += a;
                            acc_luma += luma * a;
                            hits += 1;
                        }
                    }
                }

                if hits > 0 {
                    // Normalize by total samples (Monte Carlo integration)
                    let avg_a = acc_a * inv_samples;
                    let avg_luma_premul = acc_luma * inv_samples;

                    if avg_a > 0.0 {
                        // Standard compositing derivation for accumulated samples:
                        // Emission = 16.0 * alpha + (luma/255.0) * 219.0 * alpha
                        // Here avg_luma_premul = sum(luma * alpha) / N
                        // So the second term is avg_luma_premul * (219/255)

                        let avg_emission_y = 16.0 * avg_a + avg_luma_premul * (219.0 / 255.0);
                        let dst_y = row[x] as f64;
                        let final_y = avg_emission_y + dst_y * (1.0 - avg_a);

                        row[x] = final_y.round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
        });
}

/// Compute cursor position from a cursor path (hover/mouseMove-aware), while retaining
/// click-based depress animation when click keyframes are provided.
///
/// `path_keyframes` must be sorted ascending by `video_timestamp_ms`.
/// `click_keyframes` should be sorted ascending by `video_timestamp_ms`.
pub fn compute_cursor_state_from_path(
    time_ms: f64,
    path_keyframes: &[CursorPathKeyframe],
    click_keyframes: &[ClickEffectKeyframe],
) -> Option<CursorState> {
    if path_keyframes.is_empty() {
        // Fallback to legacy click-driven cursor motion.
        return compute_cursor_state(time_ms, click_keyframes);
    }

    let depress_scale = compute_cursor_state(time_ms, click_keyframes)
        .map(|s| s.depress_scale)
        .unwrap_or(1.0);

    let first = &path_keyframes[0];
    if time_ms <= first.video_timestamp_ms {
        return Some(CursorState {
            x: first.x,
            y: first.y,
            depress_scale,
        });
    }

    let last = path_keyframes.last().unwrap_or(first);
    if time_ms >= last.video_timestamp_ms {
        return Some(CursorState {
            x: last.x,
            y: last.y,
            depress_scale,
        });
    }

    // Screen Studio-style motion between anchors:
    // Hold at the current anchor until we are close enough to the next anchor,
    // then ease into the next anchor (arriving exactly at its timestamp).
    let mut current_idx = 0usize;
    for (idx, kf) in path_keyframes.iter().enumerate() {
        if time_ms >= kf.video_timestamp_ms {
            current_idx = idx;
        } else {
            break;
        }
    }

    let current = &path_keyframes[current_idx];
    let next = path_keyframes.get(current_idx + 1);

    let mut x = current.x;
    let mut y = current.y;

    if let Some(next) = next {
        let gap = next.video_timestamp_ms - current.video_timestamp_ms;
        if gap > 0.0 {
            let distance = ((next.x - current.x).powi(2) + (next.y - current.y).powi(2)).sqrt();
            let distance_factor = (distance / 400.0).sqrt();
            let scaled_duration = CURSOR_MOVE_MS * distance_factor.min(1.5);
            let move_duration = scaled_duration.max(1.0).min(gap * 0.8);
            let move_start = next.video_timestamp_ms - move_duration;

            if time_ms >= move_start {
                let progress = ((time_ms - move_start) / move_duration).clamp(0.0, 1.0);
                let eased = screen_studio_cursor_ease(progress);
                x = current.x + (next.x - current.x) * eased;
                y = current.y + (next.y - current.y) * eased;
            }
        }
    }

    Some(CursorState {
        x,
        y,
        depress_scale,
    })
}

/// Overlay the cursor into the chroma planes (U/V), neutralizing chroma under the cursor.
///
/// The cursor art is black/white; if we only modify luma (Y) the cursor will inherit the
/// background chroma, which can look like jagged/colored edges on saturated UI.
///
/// This is a lightweight pass that does **not** apply motion blur to chroma (we keep blur
/// on luma only for now).
pub fn overlay_cursor_uv_planes(
    u_plane: &mut [u8],
    v_plane: &mut [u8],
    width: u32,
    height: u32,
    sprite: &CursorSprite,
    cursor: CursorState,
    zoom: ZoomState,
    alpha_mul: f64,
) {
    if width == 0 || height == 0 {
        return;
    }
    if width % 2 != 0 || height % 2 != 0 {
        return;
    }

    let uv_width = width / 2;
    let uv_height = height / 2;

    let expected_uv = (uv_width as usize).saturating_mul(uv_height as usize);
    if u_plane.len() != expected_uv || v_plane.len() != expected_uv {
        return;
    }

    let alpha_mul = alpha_mul.clamp(0.0, 1.0);
    if alpha_mul <= 0.0 {
        return;
    }

    let cursor_size = (BASE_CURSOR_SIZE_PX * (width as f64 / BASE_VIDEO_WIDTH_PX))
        .round()
        .max(2.0);
    let hotspot_x = HOTSPOT_RATIO_X * cursor_size;
    let hotspot_y = HOTSPOT_RATIO_Y * cursor_size;

    let cx = width as f64 / 2.0;
    let cy = height as f64 / 2.0;
    let tip_x = cx + (cursor.x - cx) * zoom.scale + zoom.translate_x;
    let tip_y = cy + (cursor.y - cy) * zoom.scale + zoom.translate_y;

    let container_x = tip_x - hotspot_x * zoom.scale;
    let container_y = tip_y - hotspot_y * zoom.scale;

    let icon_scale = (cursor.depress_scale * ICON_SCALE).max(0.01);
    let total_scale = zoom.scale * icon_scale;
    if total_scale <= 0.0 {
        return;
    }

    let container_size = cursor_size * zoom.scale;
    let container_center_x = container_x + container_size / 2.0;
    let container_center_y = container_y + container_size / 2.0;

    let icon_size = cursor_size * icon_scale * zoom.scale;
    let icon_x = container_center_x - icon_size / 2.0;
    let icon_y = container_center_y - icon_size / 2.0;

    let base_w = sprite.width() as f64;
    let base_h = sprite.height() as f64;
    let scale_x = icon_size / base_w.max(1.0);
    let scale_y = icon_size / base_h.max(1.0);

    // UV bounding box in UV coordinates.
    let uv_x0 = (icon_x / 2.0).floor() as i64;
    let uv_y0 = (icon_y / 2.0).floor() as i64;
    let uv_x1 = ((icon_x + icon_size) / 2.0).ceil() as i64;
    let uv_y1 = ((icon_y + icon_size) / 2.0).ceil() as i64;

    let start_x = uv_x0.clamp(0, uv_width as i64) as usize;
    let end_x = uv_x1.clamp(0, uv_width as i64) as usize;
    let start_y = uv_y0.clamp(0, uv_height as i64) as usize;
    let end_y = uv_y1.clamp(0, uv_height as i64) as usize;

    if start_x >= end_x || start_y >= end_y {
        return;
    }

    let row_len = uv_width as usize;
    const NEUTRAL_CHROMA: f64 = 128.0;

    for uy in start_y..end_y {
        let row_start = uy.saturating_mul(row_len);
        let u_row = &mut u_plane[row_start..row_start + row_len];
        let v_row = &mut v_plane[row_start..row_start + row_len];

        for ux in start_x..end_x {
            let base_x = (ux as f64) * 2.0;
            let base_y = (uy as f64) * 2.0;

            // Average alpha over the 2x2 luma footprint that this UV sample covers.
            let mut acc_a = 0.0;
            for dy in [0.5f64, 1.5f64] {
                for dx in [0.5f64, 1.5f64] {
                    let x_f = base_x + dx;
                    let y_f = base_y + dy;
                    let sx = (x_f - icon_x) / scale_x.max(1e-9);
                    let sy = (y_f - icon_y) / scale_y.max(1e-9);
                    if sx >= 0.0 && sy >= 0.0 && sx < base_w && sy < base_h {
                        let src = sprite.sample_bilinear(sx, sy);
                        let a = (src[3] as f64 / 255.0) * alpha_mul;
                        acc_a += a;
                    }
                }
            }

            let a = (acc_a * 0.25).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }

            let du = u_row[ux] as f64;
            let dv = v_row[ux] as f64;
            u_row[ux] = (NEUTRAL_CHROMA * a + du * (1.0 - a))
                .round()
                .clamp(0.0, 255.0) as u8;
            v_row[ux] = (NEUTRAL_CHROMA * a + dv * (1.0 - a))
                .round()
                .clamp(0.0, 255.0) as u8;
        }
    }
}

fn cursor_tip_in_frame(
    width: u32,
    height: u32,
    x: f64,
    y: f64,
    zoom_scale: f64,
    zoom_translate_x: f64,
    zoom_translate_y: f64,
) -> (f64, f64) {
    let cx = width as f64 / 2.0;
    let cy = height as f64 / 2.0;
    (
        cx + (x - cx) * zoom_scale + zoom_translate_x,
        cy + (y - cy) * zoom_scale + zoom_translate_y,
    )
}

fn overlay_cursor_impl_y_plane(
    y_plane: &mut [u8],
    width: u32,
    height: u32,
    sprite: &CursorSprite,
    cursor: CursorState,
    zoom_scale: f64,
    zoom_translate_x: f64,
    zoom_translate_y: f64,
    tip_override: Option<(f64, f64)>,
    alpha_mul: f64,
) {
    if width == 0 || height == 0 {
        return;
    }

    let alpha_mul = alpha_mul.clamp(0.0, 1.0);
    if alpha_mul <= 0.0 {
        return;
    }

    let cursor_size = (BASE_CURSOR_SIZE_PX * (width as f64 / BASE_VIDEO_WIDTH_PX))
        .round()
        .max(2.0);
    let hotspot_x = HOTSPOT_RATIO_X * cursor_size;
    let hotspot_y = HOTSPOT_RATIO_Y * cursor_size;

    let cx = width as f64 / 2.0;
    let cy = height as f64 / 2.0;

    let (tip_x, tip_y) = tip_override.unwrap_or_else(|| {
        (
            cx + (cursor.x - cx) * zoom_scale + zoom_translate_x,
            cy + (cursor.y - cy) * zoom_scale + zoom_translate_y,
        )
    });

    let container_x = tip_x - hotspot_x * zoom_scale;
    let container_y = tip_y - hotspot_y * zoom_scale;

    let icon_scale = (cursor.depress_scale * ICON_SCALE).max(0.01);
    let total_scale = zoom_scale * icon_scale;
    if total_scale <= 0.0 {
        return;
    }

    let container_size = cursor_size * zoom_scale;
    let container_center_x = container_x + container_size / 2.0;
    let container_center_y = container_y + container_size / 2.0;

    let icon_size = cursor_size * icon_scale * zoom_scale;
    let icon_x = container_center_x - icon_size / 2.0;
    let icon_y = container_center_y - icon_size / 2.0;

    let base_w = sprite.width() as f64;
    let base_h = sprite.height() as f64;
    let scale_x = icon_size / base_w.max(1.0);
    let scale_y = icon_size / base_h.max(1.0);

    let x0 = icon_x.floor() as i64;
    let y0 = icon_y.floor() as i64;
    let x1 = (icon_x + icon_size).ceil() as i64;
    let y1 = (icon_y + icon_size).ceil() as i64;

    let row_len = width as usize;
    for y in y0..y1 {
        if y < 0 || y >= height as i64 {
            continue;
        }
        let y_u = y as usize;
        let row_start = y_u.saturating_mul(row_len);
        let row = &mut y_plane[row_start..row_start + row_len];

        for x in x0..x1 {
            if x < 0 || x >= width as i64 {
                continue;
            }
            let sx = (x as f64 + 0.5 - icon_x) / scale_x.max(1e-9);
            let sy = (y as f64 + 0.5 - icon_y) / scale_y.max(1e-9);
            if sx < 0.0 || sy < 0.0 || sx >= base_w || sy >= base_h {
                continue;
            }
            let src = sprite.sample_bilinear(sx, sy);
            if src[3] == 0 {
                continue;
            }

            let a = (src[3] as f64 / 255.0) * alpha_mul;
            if a <= 0.0 {
                continue;
            }

            let luma =
                0.2126 * (src[0] as f64) + 0.7152 * (src[1] as f64) + 0.0722 * (src[2] as f64);
            let src_y = 16.0 + luma * (219.0 / 255.0);

            let x_u = x as usize;
            let dst_y = row[x_u] as f64;
            row[x_u] = (src_y * a + dst_y * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
        }
    }
}

fn is_actual_click(click_type: ClickType) -> bool {
    click_type != ClickType::Unspecified
}

fn depress_scale(time_since_click_ms: Option<f64>) -> f64 {
    let Some(t) = time_since_click_ms else {
        return 1.0;
    };

    let total_press_time = DEPRESS_ANTICIPATION_MS + DEPRESS_MS;
    let total_time = total_press_time + RELEASE_MS;

    if t < -DEPRESS_ANTICIPATION_MS || t > total_time {
        return 1.0;
    }

    if t < 0.0 {
        let progress = (t + DEPRESS_ANTICIPATION_MS) / DEPRESS_ANTICIPATION_MS;
        let eased = progress * progress;
        return 1.0 - (1.0 - DEPRESS_SCALE) * eased * 0.5;
    }

    if t < DEPRESS_MS {
        let progress = t / DEPRESS_MS;
        let eased = 1.0 - (1.0 - progress).powi(2);
        return 1.0 - (1.0 - DEPRESS_SCALE) * (0.5 + eased * 0.5);
    }

    let release_progress = (t - DEPRESS_MS) / RELEASE_MS;
    let eased = 1.0 - (1.0 - release_progress).powi(3);
    DEPRESS_SCALE + (1.0 - DEPRESS_SCALE) * eased
}

fn load_cursor_sprite_from_png() -> RgbaFrame {
    // Parse SVG
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_str(CURSOR_SVG, &opt).expect("Failed to parse cursor.svg");

    // Render at high resolution (128x128) for quality
    let size = 128u32;
    let pixmap_size = tiny_skia::IntSize::from_wh(size, size).expect("Invalid cursor size");

    let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height())
        .expect("Failed to create pixmap");

    // Calculate scale to fit SVG viewbox into our target size
    let svg_size = tree.size();
    let scale_x = size as f32 / svg_size.width();
    let scale_y = size as f32 / svg_size.height();
    let scale = scale_x.min(scale_y);

    let transform = tiny_skia::Transform::from_scale(scale, scale);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Convert tiny-skia RGBA to our RgbaFrame
    let mut frame = RgbaFrame::new(size, size);
    frame.data.copy_from_slice(pixmap.data());

    // Rotate -90 degrees to match Remotion's rotation
    rotate_frame_ccw_90(&frame)
}

fn rotate_frame_ccw_90(frame: &RgbaFrame) -> RgbaFrame {
    // Rotate 90 degrees counter-clockwise (which is -90 degrees)
    let mut rotated = RgbaFrame::new(frame.height, frame.width);

    for y in 0..frame.height {
        for x in 0..frame.width {
            let src_pixel = frame.get_pixel(x, y);
            // Map (x, y) -> (y, width - 1 - x) for 90° CCW
            let new_x = y;
            let new_y = frame.width - 1 - x;
            rotated.set_pixel(new_x, new_y, src_pixel);
        }
    }

    rotated
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::types::ClickEffectKeyframe;
    use crate::plan::types::CursorType;

    #[test]
    fn depress_scale_matches_expected_ranges() {
        assert!((depress_scale(None) - 1.0).abs() < 1e-9);
        assert!(depress_scale(Some(-60.0)) > 0.99);
        assert!(depress_scale(Some(-25.0)) < 1.0);
        assert!(depress_scale(Some(40.0)) < 0.9);
        assert!(depress_scale(Some(200.0)) > DEPRESS_SCALE);
        assert!((depress_scale(Some(500.0)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cursor_state_before_first_move_sticks_to_first() {
        let keyframes = vec![ClickEffectKeyframe {
            video_timestamp_ms: 1000.0,
            x: 10.0,
            y: 20.0,
            click_type: ClickType::Single,
            action_index: 0,
            has_modifiers: false,
        }];

        let state = compute_cursor_state(200.0, &keyframes).unwrap();
        assert_eq!(state.x, 10.0);
        assert_eq!(state.y, 20.0);
        assert_eq!(state.depress_scale, 1.0);
    }

    #[test]
    fn sprite_has_nonzero_alpha() {
        let sprite = CursorSprite::new();
        assert!(sprite.base.data.iter().any(|&b| b != 0));
        let any_alpha = sprite.base.data.chunks_exact(4).any(|px| px[3] > 0);
        assert!(any_alpha);
    }

    #[test]
    fn cursor_state_from_path_eases_into_next_anchor() {
        let path = vec![
            CursorPathKeyframe {
                video_timestamp_ms: 0.0,
                x: 0.0,
                y: 0.0,
                cursor_type: CursorType::Arrow,
                velocity: 0.0,
            },
            CursorPathKeyframe {
                video_timestamp_ms: 100.0,
                x: 100.0,
                y: 0.0,
                cursor_type: CursorType::Arrow,
                velocity: 0.0,
            },
        ];

        // With a small gap, move starts near the end (lead-up), not immediately.
        let before_move = compute_cursor_state_from_path(10.0, &path, &[]).unwrap();
        assert!((before_move.x - 0.0).abs() < 1e-9);

        let mid_move = compute_cursor_state_from_path(60.0, &path, &[]).unwrap();
        assert!(mid_move.x > 0.0 && mid_move.x < 100.0);

        let at_end = compute_cursor_state_from_path(100.0, &path, &[]).unwrap();
        assert!((at_end.x - 100.0).abs() < 1e-6);
        assert!((at_end.y - 0.0).abs() < 1e-6);
        assert!((at_end.depress_scale - 1.0).abs() < 1e-9);
    }
}
