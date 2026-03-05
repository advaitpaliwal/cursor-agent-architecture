#[derive(Debug, Clone, Copy)]
pub struct LensWarpParams {
    pub focal_point: (f64, f64),
    pub perspective: f64,
    pub rotate_x_deg: f64,
    pub rotate_y_deg: f64,
}

pub fn compute_lens_warp_params(
    zoom_level: f64,
    focal_point: (f64, f64),
) -> Option<LensWarpParams> {
    if zoom_level <= 1.01 {
        return None;
    }

    let perspective = lerp_clamped(zoom_level, 1.0, 2.5, 2500.0, 1000.0);

    let max_rotation = 0.3;
    let rotate_x_base = tri_lerp_clamped(
        focal_point.1,
        0.0,
        0.5,
        1.0,
        max_rotation,
        0.0,
        -max_rotation,
    );
    let rotate_y_base = tri_lerp_clamped(
        focal_point.0,
        0.0,
        0.5,
        1.0,
        -max_rotation,
        0.0,
        max_rotation,
    );

    let scale = zoom_level - 1.0;
    Some(LensWarpParams {
        focal_point,
        perspective,
        rotate_x_deg: rotate_x_base * scale,
        rotate_y_deg: rotate_y_base * scale,
    })
}

/// Map an output UV (0-1) to a source UV (0-1) by applying a subtle perspective warp.
///
/// This approximates `SimpleLensWarpContainer` (CSS perspective + rotateX/rotateY).
#[allow(dead_code)]
pub fn map_uv(
    uv: (f64, f64),
    params: LensWarpParams,
    width: u32,
    height: u32,
) -> Option<(f64, f64)> {
    if width == 0 || height == 0 {
        return None;
    }

    let (u, v) = uv;
    let (fx, fy) = params.focal_point;

    let rx = params.rotate_x_deg.to_radians();
    let ry = params.rotate_y_deg.to_radians();

    let w = width as f64;
    let h = height as f64;
    let cx = (u - fx) * w;
    let cy = (v - fy) * h;

    let z_offset = cx * ry.sin() + cy * rx.sin();
    let denom = 1.0 + z_offset / params.perspective.max(1.0);
    if denom.abs() < 1e-9 {
        return None;
    }

    let scale = 1.0 / denom;
    let warped_u = fx + (cx * scale) / w;
    let warped_v = fy + (cy * scale) / h;

    if !(0.0..=1.0).contains(&warped_u) || !(0.0..=1.0).contains(&warped_v) {
        return None;
    }
    Some((warped_u, warped_v))
}

fn lerp_clamped(x: f64, x0: f64, x1: f64, y0: f64, y1: f64) -> f64 {
    if (x1 - x0).abs() < 1e-9 {
        return y0;
    }
    let t = ((x - x0) / (x1 - x0)).clamp(0.0, 1.0);
    y0 + (y1 - y0) * t
}

fn tri_lerp_clamped(x: f64, x0: f64, x1: f64, x2: f64, y0: f64, y1: f64, y2: f64) -> f64 {
    if x <= x1 {
        lerp_clamped(x, x0, x1, y0, y1)
    } else {
        lerp_clamped(x, x1, x2, y1, y2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_warp_when_not_zoomed() {
        assert!(compute_lens_warp_params(1.0, (0.5, 0.5)).is_none());
    }

    #[test]
    fn warp_maps_inside_frame_for_center() {
        let params = compute_lens_warp_params(1.5, (0.5, 0.5)).unwrap();
        let mapped = map_uv((0.5, 0.5), params, 1920, 1080).unwrap();
        assert!((mapped.0 - 0.5).abs() < 1e-9);
        assert!((mapped.1 - 0.5).abs() < 1e-9);
    }
}
