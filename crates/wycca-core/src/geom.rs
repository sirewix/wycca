//! Polar geometry, emitted as SVG path data. Angles are radians, 0 at 12 o'clock,
//! increasing clockwise. Rings that run the other way (the year) pass negatives.

use std::f64::consts::PI;

pub fn rad(deg: f64) -> f64 {
    deg.to_radians()
}

/// Apply `reversed` flag to angle: if reversed, negate the angle direction.
/// This ensures flipping a ring cannot be half-applied; all angles pass through
/// this helper so the reversal is complete.
pub fn turn(reversed: bool, deg: f64) -> f64 {
    if reversed {
        -deg
    } else {
        deg
    }
}

/// Point at radius `r` (canvas units) and angle `a`.
pub fn polar(cx: f64, cy: f64, r: f64, a: f64) -> (f64, f64) {
    (cx + r * a.sin(), cy - r * a.cos())
}

pub fn pt(x: f64, y: f64) -> String {
    format!("{x:.2} {y:.2}")
}

fn at(cx: f64, cy: f64, r: f64, a: f64) -> String {
    let (x, y) = polar(cx, cy, r, a);
    pt(x, y)
}

/// Radial line from `r_in` to `r_out`: a marker, a tick, or a clock hand.
pub fn radial(cx: f64, cy: f64, r_in: f64, r_out: f64, a: f64) -> String {
    format!("M {} L {}", at(cx, cy, r_in, a), at(cx, cy, r_out, a))
}

/// Split into <=180 degree pieces so no arc command is degenerate and a full turn
/// (progress ring at 100%) still renders as two half arcs.
fn steps(a0: f64, a1: f64) -> usize {
    (((a1 - a0).abs() / PI).ceil().max(1.0)) as usize
}

fn sweep(step: f64) -> u32 {
    u32::from(step >= 0.0)
}

/// Annulus sector from `a0` to `a1`. Signed sweep, so it works either direction.
pub fn sector(cx: f64, cy: f64, r_in: f64, r_out: f64, a0: f64, a1: f64) -> String {
    let n = steps(a0, a1);
    let step = (a1 - a0) / n as f64;
    let mut d = format!("M {}", at(cx, cy, r_in, a0));
    for i in 1..=n {
        d.push_str(&format!(
            " A {r_in:.2} {r_in:.2} 0 0 {} {}",
            sweep(step),
            at(cx, cy, r_in, a0 + step * i as f64)
        ));
    }
    d.push_str(&format!(" L {}", at(cx, cy, r_out, a1)));
    for i in 1..=n {
        d.push_str(&format!(
            " A {r_out:.2} {r_out:.2} 0 0 {} {}",
            sweep(-step),
            at(cx, cy, r_out, a1 - step * i as f64)
        ));
    }
    d.push_str(" Z");
    d
}
