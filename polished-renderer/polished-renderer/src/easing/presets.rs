use super::bezier::cubic_bezier;

pub fn screen_studio_cursor_ease(t: f64) -> f64 {
    cubic_bezier(0.19, 1.0, 0.22, 1.0, t)
}

pub fn zoom_in_ease(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(4)
}

pub fn zoom_out_ease(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t.powi(3)
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

pub fn pan_ease(t: f64) -> f64 {
    zoom_in_ease(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_in_ease_bounds() {
        assert!((zoom_in_ease(0.0) - 0.0).abs() < 1e-9);
        assert!((zoom_in_ease(1.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn zoom_out_ease_bounds() {
        assert!((zoom_out_ease(0.0) - 0.0).abs() < 1e-9);
        assert!((zoom_out_ease(1.0) - 1.0).abs() < 1e-9);
    }
}
