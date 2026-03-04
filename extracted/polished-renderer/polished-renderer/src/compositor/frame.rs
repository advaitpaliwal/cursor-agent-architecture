#[derive(Clone, Debug)]
pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl RgbaFrame {
    pub fn new(width: u32, height: u32) -> Self {
        let len = width
            .saturating_mul(height)
            .saturating_mul(4)
            .try_into()
            .unwrap_or(0usize);
        Self {
            width,
            height,
            data: vec![0; len],
        }
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let idx = ((y * self.width + x) * 4) as usize;
        if idx + 3 >= self.data.len() {
            return [0, 0, 0, 0];
        }
        [
            self.data[idx],
            self.data[idx + 1],
            self.data[idx + 2],
            self.data[idx + 3],
        ]
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        let idx = ((y * self.width + x) * 4) as usize;
        if idx + 3 >= self.data.len() {
            return;
        }
        self.data[idx] = rgba[0];
        self.data[idx + 1] = rgba[1];
        self.data[idx + 2] = rgba[2];
        self.data[idx + 3] = rgba[3];
    }

    pub fn sample_bilinear(&self, x: f64, y: f64) -> [u8; 4] {
        let width = self.width as usize;
        let height = self.height as usize;
        if width == 0 || height == 0 {
            return [0, 0, 0, 0];
        }

        let max_x = (width - 1) as f64;
        let max_y = (height - 1) as f64;

        let x = x.clamp(0.0, max_x);
        let y = y.clamp(0.0, max_y);

        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(width - 1);
        let y1 = (y0 + 1).min(height - 1);

        let wx = x - x0 as f64;
        let wy = y - y0 as f64;

        let idx00 = (y0 * width + x0) * 4;
        let idx10 = (y0 * width + x1) * 4;
        let idx01 = (y1 * width + x0) * 4;
        let idx11 = (y1 * width + x1) * 4;

        let w0 = 1.0 - wx;
        let w1 = wx;
        let h0 = 1.0 - wy;
        let h1 = wy;

        let mut out = [0u8; 4];
        for channel in 0..4usize {
            let v00 = self.data[idx00 + channel] as f64;
            let v10 = self.data[idx10 + channel] as f64;
            let v01 = self.data[idx01 + channel] as f64;
            let v11 = self.data[idx11 + channel] as f64;
            let v0 = v00 * w0 + v10 * w1;
            let v1 = v01 * w0 + v11 * w1;
            let v = v0 * h0 + v1 * h1;
            out[channel] = v.round().clamp(0.0, 255.0) as u8;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bilinear_sampling_preserves_center_on_identity() {
        let mut frame = RgbaFrame::new(3, 3);
        frame.set_pixel(1, 1, [10, 20, 30, 255]);
        let sampled = frame.sample_bilinear(1.0, 1.0);
        assert_eq!(sampled, [10, 20, 30, 255]);
    }
}
