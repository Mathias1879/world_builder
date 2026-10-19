//! Classical two-body orbits, solved analytically.
//!
//! Determinism is the whole point of this module: `libm` for every transcendental, and a
//! **fixed** iteration count in the eccentric-anomaly solve rather than a convergence
//! test, so the same inputs take the same path on every target. Twelve Newton steps from
//! the mean anomaly converge to under 1e-9 for every eccentricity the validators allow
//! (≤ 0.95), which `tests/kepler.rs` pins down.

use crate::units::G;
use std::f64::consts::{PI, TAU};
use wb_grid::Vec3;

/// Newton iterations in [`eccentric_anomaly`]. Fixed, never a convergence test.
pub const KEPLER_ITERATIONS: usize = 12;

/// The eccentricity range the solver will work in. Anything outside is clamped, so a
/// value that escaped validation bends the orbit instead of dividing by ~0.
const MAX_ECCENTRICITY: f64 = 0.95;

fn clamp_e(e: f64) -> f64 {
    if e.is_nan() {
        0.0
    } else {
        e.clamp(0.0, MAX_ECCENTRICITY)
    }
}

/// Wraps an angle into (−π, π].
pub fn wrap_angle(a: f64) -> f64 {
    if !a.is_finite() {
        return 0.0;
    }
    let mut x = libm::fmod(a, TAU);
    if x <= -PI {
        x += TAU;
    } else if x > PI {
        x -= TAU;
    }
    if x == 0.0 { 0.0 } else { x }
}

/// Orbital period in seconds for semi-major axis `a_m` about a parent of mass `parent_kg`.
///
/// Kepler's third law, `P = 2π·√(a³/GM)`. A non-positive or non-finite input gives 0,
/// meaning "does not move": the checkers report such a body, they do not divide by it.
pub fn period_s(a_m: f64, parent_kg: f64) -> f64 {
    if a_m <= 0.0 || parent_kg <= 0.0 || !a_m.is_finite() || !parent_kg.is_finite() {
        return 0.0;
    }
    TAU * (a_m * a_m * a_m / (G * parent_kg)).sqrt()
}

/// Semi-major axis in metres for an orbital period of `period_s` about `parent_kg`.
pub fn semi_major_m(period_s: f64, parent_kg: f64) -> f64 {
    if period_s <= 0.0 || parent_kg <= 0.0 || !period_s.is_finite() || !parent_kg.is_finite() {
        return 0.0;
    }
    libm::cbrt(G * parent_kg * period_s * period_s / (TAU * TAU))
}

/// Mean anomaly at time `t_s`. Epoch: every body is at mean anomaly 0 at t = 0.
pub fn mean_anomaly(t_s: f64, period_s: f64) -> f64 {
    if period_s <= 0.0 || !period_s.is_finite() || !t_s.is_finite() {
        return 0.0;
    }
    TAU * t_s / period_s
}

/// Solves Kepler's equation `M = E − e·sin E` for `E`, with exactly
/// [`KEPLER_ITERATIONS`] Newton steps.
pub fn eccentric_anomaly(mean_anomaly: f64, e: f64) -> f64 {
    let e = clamp_e(e);
    let m = wrap_angle(mean_anomaly);
    let mut ea = m;
    for _ in 0..KEPLER_ITERATIONS {
        let f = ea - e * libm::sin(ea) - m;
        let df = 1.0 - e * libm::cos(ea);
        // df ≥ 1 − 0.95 = 0.05 after the clamp, so this never blows up.
        ea -= f / df;
    }
    ea
}

/// True anomaly from eccentric anomaly.
pub fn true_anomaly(eccentric_anomaly: f64, e: f64) -> f64 {
    if !eccentric_anomaly.is_finite() {
        return 0.0;
    }
    let e = clamp_e(e);
    let s = libm::sin(eccentric_anomaly);
    let c = libm::cos(eccentric_anomaly);
    libm::atan2((1.0 - e * e).sqrt() * s, c - e)
}

/// Distance from the focus at eccentric anomaly `ea`: `r = a(1 − e·cos E)`.
pub fn radius_m(a_m: f64, e: f64, ea: f64) -> f64 {
    if !a_m.is_finite() || !ea.is_finite() {
        return 0.0;
    }
    a_m * (1.0 - clamp_e(e) * libm::cos(ea))
}

/// Position relative to the parent.
///
/// The orbit lies in the x–y plane with periapsis on +x at mean anomaly 0, then tilts by
/// `inclination_rad` about the x axis. Every orbit shares that reference direction, which
/// is what makes "both suns in the sky at once" a property of the orbits rather than a
/// free parameter.
pub fn orbit_position(a_m: f64, e: f64, inclination_rad: f64, mean_anomaly: f64) -> Vec3 {
    if a_m <= 0.0 || !a_m.is_finite() || !inclination_rad.is_finite() || !mean_anomaly.is_finite() {
        return Vec3::new(0.0, 0.0, 0.0);
    }
    let e = clamp_e(e);
    let ea = eccentric_anomaly(mean_anomaly, e);
    let r = radius_m(a_m, e, ea);
    let nu = true_anomaly(ea, e);
    let x = r * libm::cos(nu);
    let flat = r * libm::sin(nu);
    let ci = libm::cos(inclination_rad);
    let si = libm::sin(inclination_rad);
    if inclination_rad == 0.0 {
        // Exactly flat, with no rounding residue in z — the common case, and the one the
        // hemisphere-symmetry property test depends on.
        Vec3::new(x, flat, 0.0)
    } else {
        Vec3::new(x, flat * ci, flat * si)
    }
}

/// Ecliptic longitude of a direction, in (−π, π]. Zero along +x.
pub fn ecliptic_longitude(v: Vec3) -> f64 {
    if v.x == 0.0 && v.y == 0.0 {
        return 0.0;
    }
    wrap_angle(libm::atan2(v.y, v.x))
}
