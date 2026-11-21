use std::f64::consts::TAU;

use gtk::cairo;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ink {
    red: f64,
    green: f64,
    blue: f64,
}

impl Ink {
    pub const LIGHT: Self = Self {
        red: 0.98,
        green: 0.98,
        blue: 0.98,
    };
    pub const DARK: Self = Self {
        red: 0.18,
        green: 0.2,
        blue: 0.21,
    };
    const ALERT: Self = Self {
        red: 0.89,
        green: 0.15,
        blue: 0.21,
    };

    fn apply(self, context: &cairo::Context) {
        context.set_source_rgb(self.red, self.green, self.blue);
    }
}

pub struct Glyph {
    pub size: i32,
    pub argb: Vec<u8>,
}

impl Glyph {
    pub fn draw(size: i32, ink: Ink, alert: bool) -> Option<Self> {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, size, size).ok()?;
        {
            let context = cairo::Context::new(&surface).ok()?;
            paint(&context, f64::from(size), ink, alert).ok()?;
        }
        surface.flush();
        let stride = usize::try_from(surface.stride()).ok()?;
        let width = usize::try_from(size).ok()?;
        let data = surface.data().ok()?;
        let argb = (0..width)
            .flat_map(|row| {
                data[row * stride..row * stride + width * 4]
                    .as_chunks::<4>()
                    .0
            })
            .flat_map(|pixel| u32::from_ne_bytes(*pixel).to_be_bytes())
            .collect();
        Some(Self { size, argb })
    }
}

fn paint(context: &cairo::Context, side: f64, ink: Ink, alert: bool) -> Result<(), cairo::Error> {
    context.set_line_cap(cairo::LineCap::Round);
    context.set_line_join(cairo::LineJoin::Round);
    ink.apply(context);

    context.set_line_width(side * 0.085);
    context.arc(side * 0.5, side * 0.47, side * 0.36, 0.0, TAU);
    context.stroke()?;

    context.move_to(side * 0.25, side * 0.7);
    context.line_to(side * 0.13, side * 0.9);
    context.line_to(side * 0.38, side * 0.8);
    context.close_path();
    context.fill()?;

    context.set_line_width(side * 0.07);
    context.move_to(side * 0.31, side * 0.39);
    context.line_to(side * 0.39, side * 0.57);
    context.line_to(side * 0.5, side * 0.44);
    context.line_to(side * 0.61, side * 0.57);
    context.line_to(side * 0.69, side * 0.39);
    context.stroke()?;

    if alert {
        context.set_operator(cairo::Operator::Clear);
        context.arc(side * 0.8, side * 0.2, side * 0.21, 0.0, TAU);
        context.fill()?;
        context.set_operator(cairo::Operator::Over);
        Ink::ALERT.apply(context);
        context.arc(side * 0.8, side * 0.2, side * 0.16, 0.0, TAU);
        context.fill()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Glyph, Ink};

    fn pixel(glyph: &Glyph, x: usize, y: usize) -> [u8; 4] {
        let side = usize::try_from(glyph.size).unwrap();
        let start = (y * side + x) * 4;
        glyph.argb[start..start + 4].try_into().unwrap()
    }

    #[test]
    fn produces_a_square_argb_buffer() {
        let glyph = Glyph::draw(32, Ink::LIGHT, false).unwrap();
        assert_eq!(glyph.argb.len(), 32 * 32 * 4);
        assert_eq!(pixel(&glyph, 0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn marks_unread_messages_in_red() {
        let glyph = Glyph::draw(40, Ink::DARK, true).unwrap();
        let [alpha, red, green, blue] = pixel(&glyph, 32, 8);
        assert_eq!(alpha, 255);
        assert!(red > 200 && green < 80 && blue < 80);
    }
}
