/// Compute even output dimensions preserving aspect ratio.
/// Width is clamped to the source width when a desired width is provided.
pub fn compute_target_dimensions(
    source_width: u32,
    source_height: u32,
    desired_width: Option<u32>,
) -> (u32, u32) {
    let mut width = desired_width.unwrap_or(source_width).min(source_width);
    if width == 0 {
        width = 2;
    }

    let aspect = if source_width > 0 {
        source_height as f64 / source_width as f64
    } else {
        1.0
    };

    let mut height = (width as f64 * aspect).round() as u32;
    if height == 0 {
        height = 2;
    }

    if width % 2 != 0 {
        width = width.saturating_sub(1).max(2);
    }
    if height % 2 != 0 {
        height = height.saturating_sub(1).max(2);
    }

    (width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_even_dimensions() {
        let (w, h) = compute_target_dimensions(1919, 1079, Some(1919));
        assert_eq!(w % 2, 0);
        assert_eq!(h % 2, 0);
    }

    #[test]
    fn clamps_to_source_width() {
        let (w, _h) = compute_target_dimensions(1280, 720, Some(2000));
        assert_eq!(w, 1280);
    }
}
