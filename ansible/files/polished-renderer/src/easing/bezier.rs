pub fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, t: f64) -> f64 {
    const EPSILON: f64 = 1e-6;
    let mut x = t;

    for _ in 0..8 {
        let current_x = bezier_value(x1, x2, x);
        let current_slope = bezier_slope(x1, x2, x);

        if (current_x - t).abs() < EPSILON {
            break;
        }
        if current_slope.abs() < EPSILON {
            break;
        }

        x = x - (current_x - t) / current_slope;
    }

    bezier_value(y1, y2, x)
}

fn bezier_value(p1: f64, p2: f64, t: f64) -> f64 {
    let one_minus_t = 1.0 - t;
    3.0 * one_minus_t * one_minus_t * t * p1 + 3.0 * one_minus_t * t * t * p2 + t * t * t
}

fn bezier_slope(p1: f64, p2: f64, t: f64) -> f64 {
    let one_minus_t = 1.0 - t;
    3.0 * one_minus_t * one_minus_t * p1
        + 6.0 * one_minus_t * t * (p2 - p1)
        + 3.0 * t * t * (1.0 - p2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_bezier_is_bounded() {
        let y = cubic_bezier(0.19, 1.0, 0.22, 1.0, 0.5);
        assert!(y >= 0.0);
        assert!(y <= 1.0);
    }
}
