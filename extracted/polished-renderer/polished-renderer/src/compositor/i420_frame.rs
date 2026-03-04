use crate::error::{RendererError, Result};

#[derive(Clone, Debug)]
pub struct I420Frame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl I420Frame {
    pub fn new(width: u32, height: u32) -> Result<Self> {
        let len = i420_len(width, height)?;
        Ok(Self {
            width,
            height,
            data: vec![0u8; len],
        })
    }

    pub fn expected_len(width: u32, height: u32) -> Result<usize> {
        i420_len(width, height)
    }

    pub fn y_plane(&self) -> &[u8] {
        let y_len = (self.width as usize).saturating_mul(self.height as usize);
        &self.data[..y_len.min(self.data.len())]
    }

    pub fn y_plane_mut(&mut self) -> &mut [u8] {
        let y_len = (self.width as usize).saturating_mul(self.height as usize);
        let len = y_len.min(self.data.len());
        &mut self.data[..len]
    }

    pub fn u_plane(&self) -> &[u8] {
        let (u_offset, u_len) = uv_offsets(self.width, self.height);
        let start = u_offset.min(self.data.len());
        let end = start.saturating_add(u_len).min(self.data.len());
        &self.data[start..end]
    }

    pub fn u_plane_mut(&mut self) -> &mut [u8] {
        let (u_offset, u_len) = uv_offsets(self.width, self.height);
        let start = u_offset.min(self.data.len());
        let end = start.saturating_add(u_len).min(self.data.len());
        &mut self.data[start..end]
    }

    pub fn v_plane(&self) -> &[u8] {
        let (u_offset, u_len) = uv_offsets(self.width, self.height);
        let start = u_offset.saturating_add(u_len).min(self.data.len());
        let end = start.saturating_add(u_len).min(self.data.len());
        &self.data[start..end]
    }

    pub fn v_plane_mut(&mut self) -> &mut [u8] {
        let (u_offset, u_len) = uv_offsets(self.width, self.height);
        let start = u_offset.saturating_add(u_len).min(self.data.len());
        let end = start.saturating_add(u_len).min(self.data.len());
        &mut self.data[start..end]
    }

    pub fn uv_width(&self) -> u32 {
        self.width / 2
    }

    pub fn uv_height(&self) -> u32 {
        self.height / 2
    }
}

pub(crate) fn sample_bilinear_u8(plane: &[u8], width: u32, height: u32, x: f64, y: f64) -> u8 {
    if width == 0 || height == 0 {
        return 0;
    }
    let w = width as usize;
    let h = height as usize;
    let len = w.saturating_mul(h);
    if plane.len() < len {
        return 0;
    }
    let max_x = (w - 1) as f64;
    let max_y = (h - 1) as f64;

    let x = x.clamp(0.0, max_x);
    let y = y.clamp(0.0, max_y);

    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);

    let wx = x - x0 as f64;
    let wy = y - y0 as f64;

    let idx00 = y0 * w + x0;
    let idx10 = y0 * w + x1;
    let idx01 = y1 * w + x0;
    let idx11 = y1 * w + x1;
    let v00 = plane[idx00] as f64;
    let v10 = plane[idx10] as f64;
    let v01 = plane[idx01] as f64;
    let v11 = plane[idx11] as f64;

    let w0 = 1.0 - wx;
    let w1 = wx;
    let h0 = 1.0 - wy;
    let h1 = wy;

    let v0 = v00 * w0 + v10 * w1;
    let v1 = v01 * w0 + v11 * w1;
    let v = v0 * h0 + v1 * h1;
    v.round().clamp(0.0, 255.0) as u8
}

fn i420_len(width: u32, height: u32) -> Result<usize> {
    if width == 0 || height == 0 {
        return Err(RendererError::InvalidArgument(
            "I420Frame requires non-zero dimensions".into(),
        ));
    }
    if width % 2 != 0 || height % 2 != 0 {
        return Err(RendererError::InvalidArgument(format!(
            "I420Frame requires even dimensions (got {width}x{height})"
        )));
    }

    let y_len = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| RendererError::Validation("I420 frame size overflow".into()))?;
    let uv_len = (width as usize / 2)
        .checked_mul(height as usize / 2)
        .ok_or_else(|| RendererError::Validation("I420 chroma size overflow".into()))?;
    y_len
        .checked_add(uv_len)
        .and_then(|v| v.checked_add(uv_len))
        .ok_or_else(|| RendererError::Validation("I420 frame size overflow".into()))
}

fn uv_offsets(width: u32, height: u32) -> (usize, usize) {
    let y_len = (width as usize).saturating_mul(height as usize);
    let uv_len = (width as usize / 2).saturating_mul(height as usize / 2);
    (y_len, uv_len)
}
