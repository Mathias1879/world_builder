//! Reading the entities into a resolved system.
//!
//! Three passes, each a prerequisite of the next: collect the raw records, prove the
//! parent graph is a tree rooted at the primary star, then resolve derived values in
//! depth order so a parent's mass is always known before a child needs it.

use crate::error::AstroError;
use crate::kepler::{period_s, semi_major_m};
use crate::kinds::{boolean, int, number};
use crate::units::{
    AU_M, BodyClass, SECONDS_PER_HOUR, SOLAR_LUMINOSITY_W, SOLAR_MASS_KG,
    luminosity_from_mass_solar, mass_from_density, mass_from_gravity,
};
use std::collections::BTreeMap;
use wb_editlog::{EntityId, EntityView, State};

pub const DEFAULT_YEAR_DAYS: f64 = 365.2422;
pub const DEFAULT_DAY_HOURS: f64 = 24.0;
pub const DEFAULT_TILT_DEG: f64 = 23.44;

/// Everything known about one body, with every derived value resolved. SI units.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyView {
    pub id: EntityId,
    pub name: String,
    pub class: BodyClass,
    pub order: i64,
    /// `None` for the primary star, which is the system's origin.
    pub parent: Option<EntityId>,
    pub a_m: f64,
    pub period_s: f64,
    pub eccentricity: f64,
    pub inclination_rad: f64,
    pub radius_m: f64,
    pub density_kgm3: f64,
    pub albedo: f64,
    pub mass_kg: f64,
    /// Radiated power in watts; `Some` only for stars.
    pub luminosity_w: Option<f64>,
    pub rings: bool,
    /// A belt's inner and outer edge in metres; `Some` only for a belt.
    pub extent_m: Option<(f64, f64)>,
    /// What the user actually typed, before derivation — the period/distance conflict
    /// checker compares the two, so it has to know which were stated.
    pub stated_a_m: Option<f64>,
    pub stated_period_s: Option<f64>,
    pub is_home: bool,
    pub is_primary_star: bool,
}

impl BodyView {
    pub fn is_star(&self) -> bool {
        self.class == BodyClass::Star
    }

    pub fn perihelion_m(&self) -> f64 {
        self.a_m * (1.0 - self.eccentricity)
    }

    pub fn aphelion_m(&self) -> f64 {
        self.a_m * (1.0 + self.eccentricity)
    }
}

/// The home world's own settings, read once.
#[derive(Clone, Copy, Debug)]
pub struct HomeSettings {
    pub year_days: f64,
    pub day_hours: f64,
    pub axial_tilt_rad: f64,
    pub eccentricity: f64,
    pub retrograde: bool,
    pub radius_m: f64,
    pub gravity_ms2: f64,
    pub order: i64,
}

impl HomeSettings {
    pub fn read(state: &State) -> HomeSettings {
        let planet = state.entity(EntityId::PLANET);
        let get = |field: &str| planet.as_ref().and_then(|p| number(p, field));
        let positive = |field: &str, default: f64| match get(field) {
            Some(x) if x.is_finite() && x > 0.0 => x,
            _ => default,
        };
        HomeSettings {
            year_days: positive("year_days", DEFAULT_YEAR_DAYS),
            day_hours: positive("day_hours", DEFAULT_DAY_HOURS),
            axial_tilt_rad: match get("axial_tilt_deg") {
                Some(d) if d.is_finite() => d.clamp(0.0, 90.0).to_radians(),
                _ => DEFAULT_TILT_DEG.to_radians(),
            },
            eccentricity: match get("eccentricity") {
                Some(e) if e.is_finite() => e.clamp(0.0, 0.95),
                _ => 0.0,
            },
            retrograde: planet
                .as_ref()
                .and_then(|p| boolean(p, "retrograde"))
                .unwrap_or(false),
            radius_m: finite_or(
                positive("radius_km", crate::units::EARTH_RADIUS_KM) * 1_000.0,
                crate::units::EARTH_RADIUS_KM * 1_000.0,
            ),
            gravity_ms2: positive("gravity_ms2", crate::units::EARTH_GRAVITY_MS2),
            order: planet
                .as_ref()
                .and_then(|p| int(p, "order"))
                .filter(|n| *n >= 1)
                .unwrap_or(1),
        }
    }

    /// Length of a local year in seconds.
    pub fn year_s(&self) -> f64 {
        self.year_days * self.day_hours * SECONDS_PER_HOUR
    }

    /// Length of a local day in seconds.
    pub fn day_s(&self) -> f64 {
        self.day_hours * SECONDS_PER_HOUR
    }
}

/// One entity, read but not yet resolved.
struct Raw {
    id: EntityId,
    name: String,
    class: BodyClass,
    order: i64,
    parent: Option<EntityId>,
    stated_a_m: Option<f64>,
    stated_period_s: Option<f64>,
    eccentricity: f64,
    inclination_rad: f64,
    radius_m: Option<f64>,
    density_kgm3: Option<f64>,
    albedo: Option<f64>,
    rings: bool,
    extent_m: Option<(f64, f64)>,
    mass_solar: Option<f64>,
    luminosity_solar: Option<f64>,
    is_primary_star: bool,
    is_home: bool,
}

fn clamped(v: Option<f64>, lo: f64, hi: f64, default: f64) -> f64 {
    match v {
        Some(x) if x.is_finite() => x.clamp(lo, hi),
        _ => default,
    }
}

fn read_positive(e: &EntityView<'_>, field: &str) -> Option<f64> {
    number(e, field).filter(|x| x.is_finite() && *x > 0.0)
}

/// A positive field converted to SI, dropped if the conversion overflows.
fn read_scaled(e: &EntityView<'_>, field: &str, scale: f64) -> Option<f64> {
    read_positive(e, field)
        .map(|x| x * scale)
        .filter(|x| x.is_finite() && *x > 0.0)
}

/// `x` if finite, otherwise `fallback` — the last line of defence against a value that
/// is finite before conversion but overflows after it, or after a later multiplication
/// (mass from density cubes the radius, for instance).
fn finite_or(x: f64, fallback: f64) -> f64 {
    if x.is_finite() { x } else { fallback }
}

fn read_body(e: &EntityView<'_>, home: &HomeSettings) -> Option<Raw> {
    let class = BodyClass::parse(match e.get("class") {
        Some(wb_editlog::Value::Text(s)) => s.as_str(),
        _ => return None,
    })?;
    let name = match e.get("name") {
        Some(wb_editlog::Value::Text(s)) => s.clone(),
        _ => return None,
    };
    let (stated_a_m, stated_period_s, extent_m) = match class {
        BodyClass::Moon => (
            read_scaled(e, "distance_km", 1_000.0),
            read_scaled(e, "period_days", home.day_s()),
            None,
        ),
        BodyClass::Belt => {
            let inner = read_scaled(e, "inner_au", AU_M)?;
            let outer = read_scaled(e, "outer_au", AU_M)?;
            let (lo, hi) = if inner <= outer {
                (inner, outer)
            } else {
                (outer, inner)
            };
            (
                Some((lo + hi) / 2.0),
                read_scaled(e, "period_years", home.year_s()),
                Some((lo, hi)),
            )
        }
        _ => (
            read_scaled(e, "distance_au", AU_M),
            read_scaled(e, "period_years", home.year_s()),
            None,
        ),
    };
    Some(Raw {
        id: e.id,
        name,
        class,
        order: int(e, "order").filter(|n| *n >= 1).unwrap_or(1),
        parent: match e.get("parent") {
            Some(wb_editlog::Value::Entity(p)) => Some(*p),
            _ => None,
        },
        stated_a_m,
        stated_period_s,
        eccentricity: clamped(number(e, "eccentricity"), 0.0, 0.95, 0.0),
        inclination_rad: clamped(number(e, "inclination_deg"), -90.0, 90.0, 0.0).to_radians(),
        radius_m: read_scaled(e, "radius_km", 1_000.0),
        density_kgm3: read_positive(e, "density_kgm3"),
        albedo: number(e, "albedo")
            .filter(|a| a.is_finite())
            .map(|a| a.clamp(0.0, 1.0)),
        rings: boolean(e, "rings").unwrap_or(false),
        extent_m,
        mass_solar: None,
        luminosity_solar: None,
        is_primary_star: false,
        is_home: false,
    })
}

fn read_star(e: &EntityView<'_>, home: &HomeSettings) -> Option<Raw> {
    let name = match e.get("name") {
        Some(wb_editlog::Value::Text(s)) => s.clone(),
        _ => return None,
    };
    let role = match e.get("role") {
        Some(wb_editlog::Value::Text(s)) => s.clone(),
        _ => return None,
    };
    let is_primary = role == "primary";
    if !is_primary && role != "companion" {
        return None;
    }
    Some(Raw {
        id: e.id,
        name,
        class: BodyClass::Star,
        order: int(e, "order").filter(|n| *n >= 1).unwrap_or(1),
        parent: match e.get("parent") {
            Some(wb_editlog::Value::Entity(p)) => Some(*p),
            _ => None,
        },
        stated_a_m: read_scaled(e, "distance_au", AU_M),
        stated_period_s: read_scaled(e, "period_years", home.year_s()),
        eccentricity: clamped(number(e, "eccentricity"), 0.0, 0.95, 0.0),
        inclination_rad: clamped(number(e, "inclination_deg"), -90.0, 90.0, 0.0).to_radians(),
        radius_m: read_scaled(e, "radius_km", 1_000.0),
        density_kgm3: read_positive(e, "density_kgm3"),
        albedo: Some(0.0),
        rings: false,
        extent_m: None,
        mass_solar: read_positive(e, "mass_solar"),
        luminosity_solar: read_positive(e, "luminosity_solar"),
        is_primary_star: is_primary,
        is_home: false,
    })
}

fn home_raw(home: &HomeSettings, state: &State) -> Raw {
    let name = state
        .entity(EntityId::PLANET)
        .and_then(|p| match p.get("name") {
            Some(wb_editlog::Value::Text(s)) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "the home world".to_string());
    Raw {
        id: EntityId::PLANET,
        name,
        class: BodyClass::Home,
        order: home.order,
        parent: None, // set to the primary star during resolution
        stated_a_m: None,
        stated_period_s: Some(home.year_s()),
        eccentricity: home.eccentricity,
        inclination_rad: 0.0,
        radius_m: Some(home.radius_m),
        density_kgm3: None,
        albedo: Some(BodyClass::Home.defaults().albedo),
        rings: false,
        extent_m: None,
        mass_solar: None,
        luminosity_solar: None,
        is_primary_star: false,
        is_home: true,
    }
}

/// Reads and resolves every body. Returns the bodies in `EntityId` order, with the
/// primary star's id.
pub fn resolve(
    state: &State,
    home: &HomeSettings,
) -> Result<(Vec<BodyView>, EntityId), AstroError> {
    let mut raws: BTreeMap<EntityId, Raw> = BTreeMap::new();
    let mut primaries = Vec::new();
    for e in state.live() {
        match e.kind() {
            Some("astro.body") => {
                if let Some(r) = read_body(&e, home) {
                    raws.insert(e.id, r);
                }
            }
            Some("astro.star") => {
                if let Some(r) = read_star(&e, home) {
                    if r.is_primary_star {
                        primaries.push(e.id);
                    }
                    raws.insert(e.id, r);
                }
            }
            _ => {}
        }
    }
    let primary = match primaries.len() {
        0 => return Err(AstroError::NoPrimaryStar),
        1 => primaries[0],
        _ => return Err(AstroError::MultiplePrimaryStars),
    };
    raws.insert(EntityId::PLANET, home_raw(home, state));

    // Every node's parent defaults to the primary star; the primary itself has none.
    let ids: Vec<EntityId> = raws.keys().copied().collect();
    for id in &ids {
        let parent = raws[id].parent;
        let resolved = if *id == primary {
            None
        } else {
            match parent {
                None => Some(primary),
                Some(p) if p == *id => return Err(AstroError::ParentCycle(*id)),
                Some(p) if raws.contains_key(&p) => Some(p),
                Some(_) => return Err(AstroError::MissingParent(*id)),
            }
        };
        raws.get_mut(id).expect("id came from the map").parent = resolved;
    }

    // Depth from the primary star, which also proves there is no cycle.
    let limit = ids.len() + 1;
    let mut depth: BTreeMap<EntityId, usize> = BTreeMap::new();
    for id in &ids {
        let mut steps = 0;
        let mut cursor = *id;
        while let Some(p) = raws[&cursor].parent {
            steps += 1;
            if steps > limit {
                return Err(AstroError::ParentCycle(*id));
            }
            cursor = p;
        }
        depth.insert(*id, steps);
    }

    // Resolve shallowest first, so a parent's mass is known before its children need it.
    let mut order: Vec<EntityId> = ids.clone();
    order.sort_by_key(|id| (depth[id], *id));

    let mut out: BTreeMap<EntityId, BodyView> = BTreeMap::new();
    for id in order {
        let r = &raws[&id];
        let defaults = r.class.defaults();
        let radius_m = finite_or(
            r.radius_m.unwrap_or(defaults.radius_km * 1_000.0),
            defaults.radius_km * 1_000.0,
        );
        let density_kgm3 = r.density_kgm3.unwrap_or(defaults.density_kgm3);
        let (mass_kg, luminosity_w) = match r.class {
            BodyClass::Star => {
                let m = r.mass_solar.unwrap_or(1.0);
                let l = r
                    .luminosity_solar
                    .unwrap_or_else(|| luminosity_from_mass_solar(m));
                (
                    finite_or(m * SOLAR_MASS_KG, 0.0),
                    Some(finite_or(l * SOLAR_LUMINOSITY_W, 0.0)),
                )
            }
            BodyClass::Home => (
                finite_or(mass_from_gravity(home.gravity_ms2, radius_m), 0.0),
                None,
            ),
            // A belt's mass is spread over its whole annulus; nothing orbits it and it
            // perturbs nothing in a two-body model.
            BodyClass::Belt => (0.0, None),
            _ => (
                finite_or(mass_from_density(radius_m, density_kgm3), 0.0),
                None,
            ),
        };
        let parent_mass = r
            .parent
            .and_then(|p| out.get(&p))
            .map(|p| p.mass_kg)
            .unwrap_or(0.0);
        // Both stated: the distance drives the geometry and the clash is a verdict.
        let (a_m, period) = match (r.stated_a_m, r.stated_period_s) {
            (Some(a), _) => (a, period_s(a, parent_mass)),
            (None, Some(p)) => (semi_major_m(p, parent_mass), p),
            (None, None) => (0.0, 0.0),
        };
        let period_s_final = if r.parent.is_none() { 0.0 } else { period };
        out.insert(
            id,
            BodyView {
                id: r.id,
                name: r.name.clone(),
                class: r.class,
                order: r.order,
                parent: r.parent,
                a_m: if r.parent.is_none() { 0.0 } else { a_m },
                period_s: period_s_final,
                eccentricity: r.eccentricity,
                inclination_rad: r.inclination_rad,
                radius_m,
                density_kgm3,
                albedo: r.albedo.unwrap_or(defaults.albedo),
                mass_kg,
                luminosity_w,
                rings: r.rings,
                extent_m: r.extent_m,
                stated_a_m: r.stated_a_m,
                stated_period_s: r.stated_period_s,
                is_home: r.is_home,
                is_primary_star: r.is_primary_star,
            },
        );
    }
    Ok((out.into_values().collect(), primary))
}
