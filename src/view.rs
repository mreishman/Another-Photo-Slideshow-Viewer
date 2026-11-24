use crate::win::{Point, Rect};

#[derive(Clone, Copy, Default)]
pub enum Mode {
    #[default]
    Fit,
    Fill,
    Actual,
    Stretch,
}

impl Mode {
    pub fn next(self) -> Self {
        match self {
            Self::Fit => Self::Fill,
            Self::Fill => Self::Actual,
            Self::Actual => Self::Stretch,
            Self::Stretch => Self::Fit,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Fit => "Fit",
            Self::Fill => "Fill",
            Self::Actual => "Actual Size",
            Self::Stretch => "Stretch",
        }
    }
}

pub struct View {
    pub mode: Mode,
    zoom: f64,
    pan: Point,
}

impl Default for View {
    fn default() -> Self {
        Self {
            mode: Mode::Fit,
            zoom: 1.0,
            pan: Point::default(),
        }
    }
}

impl View {
    pub fn reset(&mut self) {
        self.zoom = 1.0;
        self.pan = Point::default();
    }

    pub fn zoom(&mut self, steps: f64) {
        self.zoom = (self.zoom * 1.25_f64.powf(steps)).clamp(0.1, 16.0);
    }

    pub fn pan(&mut self, area: Rect, width: i32, height: i32, dx: i32, dy: i32) {
        let (w, h) = self.size(area, width, height);
        self.pan.x = (self
            .pan
            .x
            .clamp(-overflow(w, area.width()), overflow(w, area.width()))
            + dx)
            .clamp(-overflow(w, area.width()), overflow(w, area.width()));
        self.pan.y = (self
            .pan
            .y
            .clamp(-overflow(h, area.height()), overflow(h, area.height()))
            + dy)
            .clamp(-overflow(h, area.height()), overflow(h, area.height()));
    }

    fn size(&self, area: Rect, width: i32, height: i32) -> (i32, i32) {
        let x = f64::from(area.width()) / f64::from(width);
        let y = f64::from(area.height()) / f64::from(height);
        let (x, y) = match self.mode {
            Mode::Fit => (x.min(y), x.min(y)),
            Mode::Fill => (x.max(y), x.max(y)),
            Mode::Actual => (1.0, 1.0),
            Mode::Stretch => (x, y),
        };
        let scale = |pixels: i32, ratio: f64| (f64::from(pixels) * ratio * self.zoom).round().max(1.0) as i32;
        (scale(width, x), scale(height, y))
    }

    pub fn destination(&self, area: Rect, width: i32, height: i32) -> Rect {
        let (w, h) = self.size(area, width, height);
        let x = overflow(w, area.width());
        let y = overflow(h, area.height());
        Rect::new(
            area.left + (area.width() - w) / 2 + self.pan.x.clamp(-x, x),
            area.top + (area.height() - h) / 2 + self.pan.y.clamp(-y, y),
            w,
            h,
        )
    }
}

fn overflow(image: i32, viewport: i32) -> i32 {
    (image - viewport).max(0) / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_scale_portrait_images_as_expected() {
        let area = Rect::new(0, 0, 800, 400);
        for (mode, expected) in [
            (Mode::Fit, (200, 400)),
            (Mode::Fill, (800, 1600)),
            (Mode::Actual, (100, 200)),
            (Mode::Stretch, (800, 400)),
        ] {
            let view = View {
                mode,
                ..View::default()
            };
            let rect = view.destination(area, 100, 200);
            assert_eq!((rect.width(), rect.height()), expected);
        }
    }

    #[test]
    fn pan_stays_within_image_and_reset_restores_base_mode() {
        let area = Rect::new(0, 0, 800, 400);
        let mut view = View {
            mode: Mode::Actual,
            ..View::default()
        };
        view.zoom(1.0);
        view.pan(area, 800, 400, 1000, -1000);
        let rect = view.destination(area, 800, 400);
        assert_eq!((rect.left, rect.bottom), (0, 400));
        view.reset();
        assert_eq!(view.destination(area, 800, 400).width(), 800);
    }
}
