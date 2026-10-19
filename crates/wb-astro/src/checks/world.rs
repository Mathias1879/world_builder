//! Checkers over the home world itself: the numbers it states about its own orbit, the
//! temperature those numbers imply, and what the tides will eventually do to its day.

use crate::checks::orbital::checker_model;
use crate::checks::suggestion;
use crate::sky::SkyModel;
use crate::units::{AU_M, G, SECONDS_PER_HOUR, STEFAN_BOLTZMANN};
use std::f64::consts::{PI, TAU};
use wb_constraints::{CheckContext, Checker, ConstraintView, Finding, KindPattern, Param};
use wb_editlog::{EntityId, Value};

/// The band the climate checker calls habitable, in kelvin (spec §6).
pub const HABITABLE_K: (f64, f64) = (250.0, 320.0);

/// Samples per year for the mean-flux integral. Fixed, like every other count here.
const FLUX_SAMPLES: usize = 36;

/// Mean flux at the home world over one year, W·m⁻², summed over every star.
pub fn mean_stellar_flux(sky: &SkyModel) -> f64 {
    let year = sky.year_days();
    let mut total = 0.0;
    for k in 0..FLUX_SAMPLES {
        let day = year * k as f64 / FLUX_SAMPLES as f64;
        for star in sky.stars() {
            let Some(luminosity) = star.luminosity_w else {
                continue;
            };
            let Some(to_star) = sky.position(star.id, day) else {
                continue;
            };
            let d = to_star.length();
            if d > 0.0 {
                let f = luminosity / (4.0 * PI * d * d);
                if f.is_finite() {
                    total += f;
                }
            }
        }
    }
    total / FLUX_SAMPLES as f64
}

/// `T = ((1 − A)·S / 4σ)^¼`.
pub fn equilibrium_temperature_k(flux: f64, albedo: f64) -> f64 {
    if !flux.is_finite() || flux <= 0.0 {
        return 0.0;
    }
    let absorbed = (1.0 - albedo.clamp(0.0, 1.0)) * flux;
    if absorbed <= 0.0 {
        return 0.0;
    }
    let t = libm::pow(absorbed / (4.0 * STEFAN_BOLTZMANN), 0.25);
    if t.is_finite() { t.max(0.0) } else { 0.0 }
}

/// Tidal-locking timescale for the home world under one perturber, in seconds.
///
/// `t ≈ ω·a⁶·I·Q / (3·G·m²·k₂·R⁵)` with `Q = 100`, `k₂ = 0.3` and `I = 0.33·M·R²` — the
/// usual rigid-body estimate. It gives Earth tens of billions of years under the Moon,
/// which is the right order, and that is all a feasibility hint needs.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn locking_time_s(sky: &SkyModel, perturber_kg: f64, distance_m: f64) -> f64 {
    let home = sky.home();
    let (r, m_home) = (home.radius_m, home.mass_kg);
    if !(perturber_kg > 0.0) || !(distance_m > 0.0) || !(r > 0.0) || !(m_home > 0.0) {
        return f64::INFINITY;
    }
    let omega = TAU / (sky.day_hours() * SECONDS_PER_HOUR);
    let inertia = 0.33 * m_home * r * r;
    let a2 = distance_m * distance_m;
    let a6 = a2 * a2 * a2;
    let r5 = r * r * r * r * r;
    let numerator = omega * a6 * inertia * 100.0;
    let denominator = 3.0 * G * perturber_kg * perturber_kg * 0.3 * r5;
    if denominator <= 0.0 {
        return f64::INFINITY;
    }
    let t = numerator / denominator;
    if t.is_finite() { t } else { f64::INFINITY }
}

/// A body that states both a period and a distance, inconsistently.
pub struct BodyCheck;

impl Checker for BodyCheck {
    fn id(&self) -> &'static str {
        "astro.body"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("astro.body")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = checker_model(ctx) else {
            return Vec::new();
        };
        let Some(body) = sky.body(c.id) else {
            return Vec::new();
        };
        let (Some(a), Some(p)) = (body.stated_a_m, body.stated_period_s) else {
            return Vec::new();
        };
        let Some(parent) = body.parent.and_then(|id| sky.body(id)) else {
            return Vec::new();
        };
        let implied = crate::kepler::period_s(a, parent.mass_kg);
        if implied <= 0.0 || (p - implied).abs() <= 0.05 * implied {
            return Vec::new();
        }
        let is_moon = body.class == crate::units::BodyClass::Moon;
        let (period_field, distance_field) = if is_moon {
            ("period_days", "distance_km")
        } else {
            ("period_years", "distance_au")
        };
        let unit = if is_moon {
            sky.day_hours() * SECONDS_PER_HOUR
        } else {
            sky.year_days() * sky.day_hours() * SECONDS_PER_HOUR
        };
        let distance_unit = if is_moon { 1_000.0 } else { AU_M };
        vec![
            Finding::issue(self.id(), "astro.period_distance_conflict", 0.7)
                .with_param("stated_period_years", Param::Float(p / unit))
                .with_param("implied_period_years", Param::Float(implied / unit))
                .with_param("stated_distance_au", Param::Float(a / distance_unit))
                .with_suggestion(suggestion(
                    "astro.keep_distance",
                    vec![("distance_au", Param::Float(a / distance_unit))],
                    vec![(body.id, period_field.to_string(), Value::Null)],
                ))
                .with_suggestion(suggestion(
                    "astro.keep_period",
                    vec![("period_years", Param::Float(p / unit))],
                    vec![(body.id, distance_field.to_string(), Value::Null)],
                )),
        ]
    }
}

/// Whether the home world could hold liquid water.
pub struct ClimateCheck;

impl Checker for ClimateCheck {
    fn id(&self) -> &'static str {
        "astro.climate"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("planet")]
    }

    fn check(&self, _c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = checker_model(ctx) else {
            return Vec::new();
        };
        let home = sky.home();
        let flux = mean_stellar_flux(&sky);
        let t = equilibrium_temperature_k(flux, home.albedo);
        if (HABITABLE_K.0..=HABITABLE_K.1).contains(&t) {
            return Vec::new();
        }
        // The year that lands mid-band, from the primary alone: the distance where its
        // flux gives the target temperature, converted back into local days.
        let target = (HABITABLE_K.0 + HABITABLE_K.1) / 2.0;
        let primary = sky.primary_star();
        let mut finding = Finding::issue(self.id(), "astro.outside_habitable_zone", 0.6)
            .with_param("temperature_k", Param::Float(t))
            .with_param("flux_wm2", Param::Float(flux))
            .with_param("albedo", Param::Float(home.albedo));
        if let Some(luminosity) = primary.luminosity_w {
            let absorbed = (1.0 - home.albedo.clamp(0.0, 1.0)).max(f64::MIN_POSITIVE);
            let needed = 4.0 * STEFAN_BOLTZMANN * libm::pow(target, 4.0) / absorbed;
            let distance = (luminosity / (4.0 * PI * needed)).sqrt();
            let period = crate::kepler::period_s(distance, primary.mass_kg);
            let year_days = period / (sky.day_hours() * SECONDS_PER_HOUR);
            if year_days.is_finite() && year_days > 0.0 {
                finding = finding.with_suggestion(suggestion(
                    "astro.retune_year",
                    vec![
                        ("year_days", Param::Float(year_days)),
                        ("distance_au", Param::Float(distance / AU_M)),
                    ],
                    vec![(
                        EntityId::PLANET,
                        "year_days".to_string(),
                        Value::Float(year_days),
                    )],
                ));
            }
        }
        vec![finding]
    }
}

/// Whether the world's day will survive its tides.
pub struct SpinCheck;

impl Checker for SpinCheck {
    fn id(&self) -> &'static str {
        "astro.spin"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("planet")]
    }

    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn check(&self, _c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = checker_model(ctx) else {
            return Vec::new();
        };
        // A civilisation's history: ten thousand local years.
        let budget_s = 10_000.0 * sky.year_days() * sky.day_hours() * SECONDS_PER_HOUR;
        let mut worst: Option<(f64, String)> = None;
        let perturbers = sky
            .moons_of_home()
            .map(|m| (m.mass_kg, m.a_m, m.name.clone()))
            .chain(sky.stars().filter_map(|s| {
                let d = sky.position(s.id, 0.0)?.length();
                Some((s.mass_kg, d, s.name.clone()))
            }));
        for (mass, distance, name) in perturbers {
            let t = locking_time_s(&sky, mass, distance);
            if worst.as_ref().map(|(best, _)| t < *best).unwrap_or(true) {
                worst = Some((t, name));
            }
        }
        let Some((t, name)) = worst else {
            return Vec::new();
        };
        if !(t < budget_s) {
            return Vec::new();
        }
        let years = t / (sky.year_days() * sky.day_hours() * SECONDS_PER_HOUR);
        vec![
            Finding::consequence(self.id(), "astro.tidal_locking")
                .with_param("years", Param::Float(years))
                .with_param("perturber", Param::Text(name)),
        ]
    }
}
