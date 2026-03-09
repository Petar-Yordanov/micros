use crate::geom::Rect;

pub struct Canvas {
    fb: *mut u32,
    pitch_pixels: usize,
    width: usize,
    height: usize,
    clip: Rect,
}

impl Canvas {
    #[inline]
    pub fn new(fb: *mut u32, pitch_pixels: usize, width: usize, height: usize) -> Self {
        let clip = Rect::new(0, 0, width as i32, height as i32);
        Self {
            fb,
            pitch_pixels,
            width,
            height,
            clip,
        }
    }

    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }

    #[inline]
    pub fn height(&self) -> usize {
        self.height
    }

    #[inline]
    pub fn set_clip(&mut self, rect: Rect) {
        let screen = Rect::new(0, 0, self.width as i32, self.height as i32);
        self.clip = intersect_rect(screen, rect).unwrap_or(Rect::new(0, 0, 0, 0));
    }

    #[inline]
    pub fn reset_clip(&mut self) {
        self.clip = Rect::new(0, 0, self.width as i32, self.height as i32);
    }

    #[inline]
    pub fn intersects(&self, rect: Rect) -> bool {
        intersect_rect(self.clip, rect).is_some()
    }

    #[inline]
    pub fn clear(&mut self, color: u32) {
        self.fill_rect(Rect::new(0, 0, self.width as i32, self.height as i32), color);
    }

    #[inline]
    pub fn put_pixel(&mut self, x: i32, y: i32, color: u32) {
        if x < self.clip.x
            || y < self.clip.y
            || x >= self.clip.right()
            || y >= self.clip.bottom()
        {
            return;
        }

        if x < 0 || y < 0 {
            return;
        }
        let x = x as usize;
        let y = y as usize;
        if x >= self.width || y >= self.height {
            return;
        }

        unsafe {
            *self.fb.add(y * self.pitch_pixels + x) = color;
        }
    }

    #[inline]
    pub fn hline(&mut self, x: i32, y: i32, w: i32, color: u32) {
        if w <= 0 {
            return;
        }
        self.fill_rect(Rect::new(x, y, w, 1), color);
    }

    #[inline]
    pub fn vline(&mut self, x: i32, y: i32, h: i32, color: u32) {
        if h <= 0 {
            return;
        }
        self.fill_rect(Rect::new(x, y, 1, h), color);
    }

    #[inline]
    pub fn fill_rect(&mut self, rect: Rect, color: u32) {
        let Some(r) = intersect_rect(rect, self.clip) else {
            return;
        };

        let x0 = r.x.max(0) as usize;
        let y0 = r.y.max(0) as usize;
        let x1 = r.right().min(self.width as i32).max(0) as usize;
        let y1 = r.bottom().min(self.height as i32).max(0) as usize;

        if x0 >= x1 || y0 >= y1 {
            return;
        }

        for yy in y0..y1 {
            unsafe {
                let row = core::slice::from_raw_parts_mut(
                    self.fb.add(yy * self.pitch_pixels + x0),
                    x1 - x0,
                );
                row.fill(color);
            }
        }
    }

    #[inline]
    pub fn fill_vertical_gradient(&mut self, rect: Rect, top: u32, bottom: u32) {
        if rect.w <= 0 || rect.h <= 0 {
            return;
        }

        if !self.intersects(rect) {
            return;
        }

        let den = (rect.h - 1).max(1) as u32;
        for row in 0..rect.h {
            let color = lerp_rgb(top, bottom, row as u32, den);
            self.fill_rect(Rect::new(rect.x, rect.y + row, rect.w, 1), color);
        }
    }

    #[inline]
    pub fn stroke_rect(&mut self, rect: Rect, color: u32) {
        if rect.w <= 0 || rect.h <= 0 {
            return;
        }

        self.fill_rect(Rect::new(rect.x, rect.y, rect.w, 1), color);
        self.fill_rect(Rect::new(rect.x, rect.bottom() - 1, rect.w, 1), color);
        self.fill_rect(Rect::new(rect.x, rect.y, 1, rect.h), color);
        self.fill_rect(Rect::new(rect.right() - 1, rect.y, 1, rect.h), color);
    }

    #[inline]
    pub fn stroke_rect_inset(&mut self, rect: Rect, inset: i32, color: u32) {
        let r = rect.inset(inset, inset);
        if r.w > 0 && r.h > 0 {
            self.stroke_rect(r, color);
        }
    }
}

fn lerp_rgb(a: u32, b: u32, num: u32, den: u32) -> u32 {
    let ar = (a >> 16) & 0xff;
    let ag = (a >> 8) & 0xff;
    let ab = a & 0xff;

    let br = (b >> 16) & 0xff;
    let bg = (b >> 8) & 0xff;
    let bb = b & 0xff;

    let rr = (ar * (den - num) + br * num) / den;
    let rg = (ag * (den - num) + bg * num) / den;
    let rb = (ab * (den - num) + bb * num) / den;

    (rr << 16) | (rg << 8) | rb
}

fn intersect_rect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = a.right().min(b.right());
    let y1 = a.bottom().min(b.bottom());

    if x1 <= x0 || y1 <= y0 {
        None
    } else {
        Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
    }
}
