//! Checkers over the system's geometry.
//!
//! Each one builds a [`SkyModel`] from `ctx.state` inside `check`. That rebuild per entity
//! is deliberate: a checker holds no state, so it cannot go stale against the world it is
//! judging, and a system is tens of bodies. If it ever shows up in the perf guard, the fix
//! is the memo slot recorded in the Constraints follow-ups, not a cache in here.

use crate::sky::SkyModel;
use crate::system::BodyView;
use crate::units::{AU_M, MIN_STELLAR_MASS_SOLAR};
use std::collections::BTreeMap;
use wb_constraints::{
    CheckContext, Checker, ConstraintView, Finding, IssueCode, KindPattern, Param, Suggestion,
};
use wb_editlog::{EntityId, Value};

/// Holman–Wiegert (1999) critical semi-major axis for an S-type orbit, as a fraction of
/// the binary separation.
///
/// Exposed separately because the fix for an unstable companion has to invert the
/// criterion: the separation that makes a given orbit safe is that orbit divided by this
/// fraction, and multiplying the separation by a guessed factor does not reliably clear
/// the finding it is offered for.
pub fn critical_fraction(mass_ratio: f64, eccentricity: f64) -> f64 {
    let u = mass_ratio.clamp(0.0, 1.0);
    let e = eccentricity.clamp(0.0, 0.95);
    let fraction =
        0.464 - 0.380 * u - 0.631 * e + 0.586 * u * e + 0.150 * e * e - 0.198 * u * e * e;
    if fraction.is_finite() {
        fraction.max(0.0)
    } else {
        0.0
    }
}

/// The critical distance itself: the separation times [`critical_fraction`].
pub fn critical_distance_m(separation_m: f64, mass_ratio: f64, eccentricity: f64) -> f64 {
    let d = separation_m * critical_fraction(mass_ratio, eccentricity);
    if d.is_finite() { d.max(0.0) } else { 0.0 }
}

/// Hill radius: `a·∛(m / 3M)`.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn hill_radius_m(a_m: f64, mass_kg: f64, primary_kg: f64) -> f64 {
    if !(a_m > 0.0) || !(mass_kg > 0.0) || !(primary_kg > 0.0) {
        return 0.0;
    }
    let r = a_m * libm::cbrt(mass_kg / (3.0 * primary_kg));
    if r.is_finite() { r.max(0.0) } else { 0.0 }
}

/// The fluid (deformable) Roche limit: `2.44·R·∛(ρ_body / ρ_satellite)`.
///
/// Any moon above a few hundred kilometres is held together by its own gravity rather
/// than material strength, so the fluid threshold — not the smaller rigid-body figure —
/// is the one that matters for a plausibility warning.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn roche_limit_m(radius_m: f64, body_density: f64, satellite_density: f64) -> f64 {
    if !(radius_m > 0.0) || !(body_density > 0.0) || !(satellite_density > 0.0) {
        return 0.0;
    }
    let d = 2.44 * radius_m * libm::cbrt(body_density / satellite_density);
    if d.is_finite() { d.max(0.0) } else { 0.0 }
}

/// Tolerance on a resonance, as a fraction of the ratio.
const RESONANCE_TOLERANCE: f64 = 0.02;

/// `Some((p, q))` when `ratio` is within 2% of `p/q` for `p`, `q` in 1..=5.
///
/// Smaller denominators win, so a 2:1 is never reported as a 4:2.
pub fn simple_ratio(ratio: f64) -> Option<(i64, i64)> {
    if !ratio.is_finite() || ratio <= 0.0 {
        return None;
    }
    let mut best: Option<(i64, i64)> = None;
    for q in 1..=5i64 {
        for p in 1..=5i64 {
            if p == q && p != 1 {
                continue;
            }
            let target = p as f64 / q as f64;
            if (ratio - target).abs() <= RESONANCE_TOLERANCE * target {
                let better = match best {
                    None => true,
                    Some((bp, bq)) => (q, p) < (bq, bp),
                };
                if better {
                    best = Some((p, q));
                }
            }
        }
    }
    best
}

fn model(ctx: &CheckContext<'_>) -> Option<SkyModel> {
    SkyModel::build(ctx.state).ok()
}

/// Bodies sharing a parent, in `EntityId` order, belts excluded.
fn siblings(sky: &SkyModel, parent: Option<EntityId>) -> Vec<&BodyView> {
    sky.bodies()
        .filter(|b| b.parent == parent && b.extent_m.is_none() && b.a_m > 0.0)
        .collect()
}

fn au(x: f64) -> Param {
    Param::Float(x / AU_M)
}

fn write(entity: EntityId, field: &str, value: Value) -> Vec<(EntityId, String, Value)> {
    vec![(entity, field.to_string(), value)]
}

fn suggestion(
    code: &str,
    params: Vec<(&str, Param)>,
    writes: Vec<(EntityId, String, Value)>,
) -> Suggestion {
    Suggestion {
        code: IssueCode::new(code),
        params: params
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect::<BTreeMap<String, Param>>(),
        writes,
        creates: Vec::new(),
    }
}

/// Companion-star stability and the period it claims.
pub struct CompanionCheck;

impl Checker for CompanionCheck {
    fn id(&self) -> &'static str {
        "astro.companion"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("astro.star")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = model(ctx) else {
            return Vec::new();
        };
        let Some(star) = sky.body(c.id) else {
            return Vec::new();
        };
        if star.is_primary_star {
            return Vec::new();
        }
        let primary = sky.primary_star();
        let mut out = Vec::new();

        // Stability: everything orbiting the primary beyond the critical distance is at
        // risk, and so is anything inside the companion's own reach.
        if star.a_m > 0.0 {
            let ratio = star.mass_kg / (star.mass_kg + primary.mass_kg).max(f64::MIN_POSITIVE);
            let fraction = critical_fraction(ratio, star.eccentricity);
            let critical = critical_distance_m(star.a_m, ratio, star.eccentricity);
            let at_risk: Vec<&BodyView> = siblings(&sky, Some(primary.id))
                .into_iter()
                .filter(|b| b.id != star.id && b.aphelion_m() > critical)
                .collect();
            if !at_risk.is_empty() {
                let outermost = at_risk
                    .iter()
                    .map(|b| b.aphelion_m())
                    .fold(0.0_f64, f64::max);
                let names = at_risk
                    .iter()
                    .map(|b| b.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                // The separation that puts the outermost body inside the critical
                // distance, with a 20% margin — inverting the criterion rather than
                // guessing a multiple, so applying the fix really does clear the finding.
                let safe = if fraction > 0.0 {
                    outermost / fraction * 1.2
                } else {
                    outermost * 10.0
                };
                let mut f = Finding::issue(self.id(), "astro.companion_destabilizes", 0.9)
                    .with_param("critical_au", au(critical))
                    .with_param("distance_au", au(star.a_m))
                    .with_param("bodies", Param::Text(names))
                    .with_param("count", Param::Int(at_risk.len() as i64))
                    .with_suggestion(suggestion(
                        "astro.move_companion_out",
                        vec![("distance_au", au(safe))],
                        write(star.id, "distance_au", Value::Float(safe / AU_M)),
                    ))
                    .with_suggestion(suggestion(
                        "astro.demote_companion",
                        vec![("mass_solar", Param::Float(MIN_STELLAR_MASS_SOLAR))],
                        write(star.id, "mass_solar", Value::Float(MIN_STELLAR_MASS_SOLAR)),
                    ));
                for b in at_risk {
                    f = f.with_related(b.id);
                }
                out.push(f);
            }
        }

        // The period it claims against the period its distance implies.
        let year_s = sky.year_days() * sky.day_hours() * crate::units::SECONDS_PER_HOUR;
        match (star.stated_a_m, star.stated_period_s) {
            (Some(a), Some(p)) => {
                let implied_period = crate::kepler::period_s(a, primary.mass_kg + star.mass_kg);
                let implied_a = crate::kepler::semi_major_m(p, primary.mass_kg + star.mass_kg);
                if implied_period > 0.0 && (p - implied_period).abs() > 0.25 * implied_period {
                    out.push(
                        Finding::issue(self.id(), "astro.companion_period_mismatch", 0.8)
                            .with_param("stated_period_years", Param::Float(p / year_s))
                            .with_param(
                                "implied_period_years",
                                Param::Float(implied_period / year_s),
                            )
                            .with_param("stated_distance_au", au(a))
                            .with_param("implied_distance_au", au(implied_a)),
                    );
                }
            }
            (None, Some(p)) => {
                let implied_a = crate::kepler::semi_major_m(p, primary.mass_kg + star.mass_kg);
                let inside = siblings(&sky, Some(primary.id))
                    .into_iter()
                    .filter(|b| b.id != star.id)
                    .any(|b| b.aphelion_m() > implied_a);
                if inside && implied_a > 0.0 {
                    out.push(
                        Finding::issue(self.id(), "astro.companion_period_mismatch", 0.8)
                            .with_param("stated_period_years", Param::Float(p / year_s))
                            .with_param("implied_period_years", Param::Float(p / year_s))
                            .with_param("implied_distance_au", au(implied_a))
                            .with_param("stated_distance_au", au(implied_a)),
                    );
                }
            }
            _ => {}
        }
        out
    }
}

/// Stated `order` against derived distance.
pub struct OrderingCheck;

impl Checker for OrderingCheck {
    fn id(&self) -> &'static str {
        "astro.ordering"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("astro.body")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = model(ctx) else {
            return Vec::new();
        };
        let Some(body) = sky.body(c.id) else {
            return Vec::new();
        };
        // Every sibling that shares this body's parent, belts and companion stars
        // included because they occupy a place in the numbering — but NOT the home
        // world. This checker never judges the planet entity (its kind is not
        // `astro.body`) and cannot renumber it, so including it only manufactures
        // mismatches no suggestion could ever clear.
        let mut group: Vec<&BodyView> = sky
            .bodies()
            .filter(|b| b.parent == body.parent && b.a_m > 0.0 && !b.is_home)
            .collect();
        if group.len() < 2 {
            return Vec::new();
        }
        let mut by_order = group.clone();
        by_order.sort_by(|a, b| (a.order, a.id).cmp(&(b.order, b.id)));
        group.sort_by(|a, b| {
            a.a_m
                .partial_cmp(&b.a_m)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.id.cmp(&b.id))
        });
        let stated = by_order.iter().position(|b| b.id == body.id);
        let actual = group.iter().position(|b| b.id == body.id);
        let (Some(stated), Some(actual)) = (stated, actual) else {
            return Vec::new();
        };
        if stated == actual {
            return Vec::new();
        }
        let places = stated.abs_diff(actual) as f64;
        // The numbers the group actually uses, in ascending order: the body at distance
        // rank k should carry the k-th smallest of them. Taking the targets from the
        // group itself means a renumbering can only permute numbers already in use.
        let mut targets: Vec<i64> = group.iter().map(|b| b.order).collect();
        targets.sort_unstable();
        let correct = targets.get(actual).copied().unwrap_or(body.order);
        let mut finding =
            Finding::issue(self.id(), "astro.order_mismatch", (places / 4.0).min(1.0))
                .with_param("stated", Param::Int(body.order))
                .with_param("places", Param::Int(places as i64))
                .with_param("suggested", Param::Int(correct));
        if correct != body.order {
            finding = finding.with_suggestion(suggestion(
                "astro.renumber",
                vec![("order", Param::Int(correct))],
                write(body.id, "order", Value::Int(correct)),
            ));
        }
        vec![finding]
    }
}

/// Orbits that cross, and resonances that excuse them.
pub struct OrbitCheck;

impl Checker for OrbitCheck {
    fn id(&self) -> &'static str {
        "astro.orbits"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("astro.body")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = model(ctx) else {
            return Vec::new();
        };
        let Some(body) = sky.body(c.id) else {
            return Vec::new();
        };
        if body.extent_m.is_some() || body.a_m <= 0.0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for other in siblings(&sky, body.parent) {
            if other.id == body.id {
                continue;
            }
            // Each pair of bodies is judged once, from the lower id. A star or the home
            // world never runs this checker itself (their kinds are not `astro.body`), so
            // a pair involving one of them is judged from whichever body does run —
            // otherwise a companion star sitting on a planet's orbit would go unreported.
            if other.id < body.id && !other.is_star() && !other.is_home {
                continue;
            }
            let overlap = body.perihelion_m() <= other.aphelion_m()
                && other.perihelion_m() <= body.aphelion_m();
            let ratio = if other.period_s > 0.0 {
                body.period_s / other.period_s
            } else {
                0.0
            };
            let resonance = simple_ratio(ratio);
            match (overlap, resonance) {
                (true, Some((p, q))) => out.push(
                    Finding::support(self.id(), "astro.resonance_protects")
                        .with_param("other", Param::Text(other.name.clone()))
                        .with_param("p", Param::Int(p))
                        .with_param("q", Param::Int(q))
                        .with_related(other.id),
                ),
                (true, None) => out.push(
                    Finding::issue(self.id(), "astro.orbit_crossing", 0.5)
                        .with_param("other", Param::Text(other.name.clone()))
                        .with_param("perihelion_au", au(body.perihelion_m()))
                        .with_param("aphelion_au", au(body.aphelion_m()))
                        .with_param("other_perihelion_au", au(other.perihelion_m()))
                        .with_param("other_aphelion_au", au(other.aphelion_m()))
                        .with_related(other.id),
                ),
                (false, Some((p, q))) => out.push(
                    Finding::support(self.id(), "astro.resonance")
                        .with_param("other", Param::Text(other.name.clone()))
                        .with_param("p", Param::Int(p))
                        .with_param("q", Param::Int(q))
                        .with_related(other.id),
                ),
                (false, None) => {}
            }
        }
        out
    }
}

/// Moons that cannot stay where they are.
pub struct MoonCheck;

impl Checker for MoonCheck {
    fn id(&self) -> &'static str {
        "astro.moons"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("astro.body")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = model(ctx) else {
            return Vec::new();
        };
        let Some(moon) = sky.body(c.id) else {
            return Vec::new();
        };
        if moon.class != crate::units::BodyClass::Moon || moon.a_m <= 0.0 {
            return Vec::new();
        }
        let Some(planet) = moon.parent.and_then(|p| sky.body(p)) else {
            return Vec::new();
        };
        let primary = sky.primary_star();
        let mut out = Vec::new();

        let hill = hill_radius_m(planet.a_m, planet.mass_kg, primary.mass_kg);
        if hill > 0.0 && moon.a_m > hill / 3.0 {
            out.push(
                Finding::issue(self.id(), "astro.moon_outside_hill", 0.9)
                    .with_param("distance_km", Param::Float(moon.a_m / 1_000.0))
                    .with_param("hill_third_km", Param::Float(hill / 3_000.0))
                    .with_related(planet.id),
            );
        }
        let roche = roche_limit_m(planet.radius_m, planet.density_kgm3, moon.density_kgm3);
        if roche > 0.0 && moon.perihelion_m() < roche {
            out.push(
                Finding::issue(self.id(), "astro.moon_inside_roche", 0.9)
                    .with_param("distance_km", Param::Float(moon.perihelion_m() / 1_000.0))
                    .with_param("roche_km", Param::Float(roche / 1_000.0))
                    .with_related(planet.id),
            );
        }
        out
    }
}

/// A belt lying across someone else's orbit.
pub struct BeltCheck;

impl Checker for BeltCheck {
    fn id(&self) -> &'static str {
        "astro.belt"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("astro.body")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = model(ctx) else {
            return Vec::new();
        };
        let Some(belt) = sky.body(c.id) else {
            return Vec::new();
        };
        let Some((inner, outer)) = belt.extent_m else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for other in siblings(&sky, belt.parent) {
            if other.perihelion_m() >= inner && other.aphelion_m() <= outer {
                out.push(
                    Finding::issue(self.id(), "astro.belt_overlaps_orbit", 0.5)
                        .with_param("other", Param::Text(other.name.clone()))
                        .with_param("inner_au", au(inner))
                        .with_param("outer_au", au(outer))
                        .with_related(other.id),
                );
            }
        }
        out
    }
}
