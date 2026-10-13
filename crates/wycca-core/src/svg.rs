//! Minimal SVG writer. Only what a flat-colour calendar needs: paths, circles,
//! rects and numeric text. Antialiasing is the renderer's job, not ours.

use std::fmt::Write;

use crate::config::{Config, Font};

pub struct Svg {
    buf: String,
    font: Font,
}

impl Svg {
    /// `background` is a colour, or "none" for a transparent canvas.
    pub fn new(cfg: &Config, background: &str) -> Self {
        let mut buf = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">",
            cfg.size, cfg.size, cfg.size, cfg.size
        );
        if background != "none" {
            let _ = write!(
                buf,
                "<rect width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                cfg.size,
                cfg.size,
                esc(background)
            );
        }
        Self {
            buf,
            font: cfg.font.clone(),
        }
    }

    pub fn path(&mut self, d: &str, fill: &str) {
        let _ = write!(self.buf, "<path d=\"{}\" fill=\"{}\"/>", d, esc(fill));
    }

    /// Ticks, markers and clock hands. A bare `M..L` has no fill area, so lines
    /// have to be strokes or they render as nothing at all.
    pub fn line(&mut self, d: &str, stroke: &str, width: f64) {
        let _ = write!(
            self.buf,
            "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{width:.2}\" stroke-linecap=\"round\"/>",
            d,
            esc(stroke)
        );
    }

    pub fn circle(&mut self, cx: f64, cy: f64, r: f64, fill: &str) {
        let _ = write!(
            self.buf,
            "<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"{r:.2}\" fill=\"{}\"/>",
            esc(fill)
        );
    }

    pub fn text(&mut self, x: f64, y: f64, size: f64, fill: &str, anchor: &str, s: &str) {
        let _ = write!(
            self.buf,
            "<text x=\"{x:.2}\" y=\"{y:.2}\" font-size=\"{size:.1}\" fill=\"{}\" \
             text-anchor=\"{anchor}\" dominant-baseline=\"central\" \
             font-family=\"{}\" font-weight=\"{}\">{}</text>",
            esc(fill),
            esc(&self.font.family),
            self.font.weight,
            esc(s)
        );
    }

    pub fn finish(mut self) -> String {
        self.buf.push_str("</svg>");
        self.buf
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
