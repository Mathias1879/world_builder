//! The kinds this subsystem owns, their field accessors, and their type validators.
//!
//! A validator rejects data that cannot be read: text where a number belongs, a class
//! nobody defined, a month of zero days. It never rejects an implausible world — a moon
//! inside its planet's Roche limit is perfectly readable data and a bad idea, and saying
//! so is a checker's job (§6), not a validator's.

use crate::units::BodyClass;
use wb_constraints::{KindCatalog, text};
use wb_editlog::{EditLog, EntityKind, EntityView, Value};

/// Every kind this subsystem owns.
///
/// `planet` is here because the home world carries the astro fields in §3.3 and has to be
/// visited by Constraints for the climate and spin checkers to ever fire. Registering it
/// is additive: the planet gets a verdict only when a catalog with this list is in use.
pub const ASTRO_KINDS: &[&str] = &[
    "astro.body",
    "astro.calendar",
    "astro.month",
    "astro.season",
    "astro.star",
    "planet",
];

/// The longest week a calendar may name.
const MAX_WEEKDAYS: usize = 100;

pub fn int(e: &EntityView<'_>, field: &str) -> Option<i64> {
    match e.get(field) {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    }
}

pub fn boolean(e: &EntityView<'_>, field: &str) -> Option<bool> {
    match e.get(field) {
        Some(Value::Bool(b)) => Some(*b),
        _ => None,
    }
}

/// A number field, accepting an `Int` where a `Float` is meant: a user typing `30` for
/// `day_hours` means 30.0, and the editor has no reason to know the difference.
pub fn number(e: &EntityView<'_>, field: &str) -> Option<f64> {
    match e.get(field) {
        Some(Value::Float(x)) => Some(*x),
        Some(Value::Int(n)) => Some(*n as f64),
        _ => None,
    }
}

/// A list-of-text field.
pub fn words<'a>(e: &EntityView<'a>, field: &str) -> Option<Vec<&'a str>> {
    match e.get(field) {
        Some(Value::List(items)) => items
            .iter()
            .map(|v| match v {
                Value::Text(s) => Some(s.as_str()),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

fn require_text(e: &EntityView<'_>, field: &str) -> Result<(), String> {
    match e.get(field) {
        Some(Value::Text(s)) if !s.is_empty() => Ok(()),
        _ => Err(format!("{field} is required and must be text")),
    }
}

fn optional_text(e: &EntityView<'_>, field: &str) -> Result<(), String> {
    match e.get(field) {
        None | Some(Value::Text(_)) => Ok(()),
        _ => Err(format!("{field} must be text")),
    }
}

fn optional_bool(e: &EntityView<'_>, field: &str) -> Result<(), String> {
    match e.get(field) {
        None | Some(Value::Bool(_)) => Ok(()),
        _ => Err(format!("{field} must be true or false")),
    }
}

fn optional_entity(e: &EntityView<'_>, field: &str) -> Result<(), String> {
    match e.get(field) {
        None | Some(Value::Entity(_)) => Ok(()),
        _ => Err(format!("{field} must reference an entity")),
    }
}

fn require_entity(e: &EntityView<'_>, field: &str) -> Result<(), String> {
    match e.get(field) {
        Some(Value::Entity(_)) => Ok(()),
        _ => Err(format!("{field} is required and must reference an entity")),
    }
}

/// A number field, optional, inside `[lo, hi]` when present.
fn range(e: &EntityView<'_>, field: &str, lo: f64, hi: f64) -> Result<(), String> {
    match e.get(field) {
        None => Ok(()),
        Some(Value::Float(_) | Value::Int(_)) => {
            let x = number(e, field).unwrap_or(f64::NAN);
            if x.is_finite() && (lo..=hi).contains(&x) {
                Ok(())
            } else {
                Err(format!("{field} must be a number in [{lo}, {hi}]"))
            }
        }
        _ => Err(format!("{field} must be a number")),
    }
}

/// A number field, optional, strictly positive and finite when present.
fn positive(e: &EntityView<'_>, field: &str) -> Result<(), String> {
    match e.get(field) {
        None => Ok(()),
        Some(Value::Float(_) | Value::Int(_)) => {
            let x = number(e, field).unwrap_or(f64::NAN);
            if x.is_finite() && x > 0.0 {
                Ok(())
            } else {
                Err(format!("{field} must be a positive number"))
            }
        }
        _ => Err(format!("{field} must be a number")),
    }
}

/// An integer field, optional, at least `min` when present.
fn at_least(e: &EntityView<'_>, field: &str, min: i64) -> Result<(), String> {
    match e.get(field) {
        None => Ok(()),
        Some(Value::Int(n)) if *n >= min => Ok(()),
        _ => Err(format!("{field} must be a whole number of at least {min}")),
    }
}

fn require_at_least(e: &EntityView<'_>, field: &str, min: i64) -> Result<(), String> {
    match int(e, field) {
        Some(n) if n >= min => Ok(()),
        _ => Err(format!(
            "{field} is required and must be a whole number of at least {min}"
        )),
    }
}

/// Orbit fields shared by bodies and companion stars.
fn orbit(e: &EntityView<'_>) -> Result<(), String> {
    positive(e, "period_years")?;
    positive(e, "distance_au")?;
    positive(e, "period_days")?;
    positive(e, "distance_km")?;
    range(e, "eccentricity", 0.0, 0.95)?;
    range(e, "inclination_deg", -90.0, 90.0)?;
    at_least(e, "order", 1)?;
    optional_entity(e, "parent")
}

fn validate_body(e: &EntityView<'_>) -> Result<(), String> {
    require_text(e, "name")?;
    require_at_least(e, "order", 1)?;
    let Some(class) = text(e, "class").and_then(BodyClass::parse) else {
        return Err("class must be one of rocky, water, gas_giant, ice_giant, belt, moon".into());
    };
    orbit(e)?;
    positive(e, "radius_km")?;
    positive(e, "density_kgm3")?;
    range(e, "albedo", 0.0, 1.0)?;
    optional_bool(e, "rings")?;
    match class {
        BodyClass::Belt => {
            positive(e, "inner_au")?;
            positive(e, "outer_au")?;
            let (inner, outer) = (number(e, "inner_au"), number(e, "outer_au"));
            match (inner, outer) {
                (Some(_), Some(_)) => Ok(()),
                _ => Err("a belt needs both inner_au and outer_au".into()),
            }
        }
        BodyClass::Moon => {
            if number(e, "period_days").is_none() && number(e, "distance_km").is_none() {
                return Err("a moon needs period_days or distance_km".into());
            }
            Ok(())
        }
        _ => {
            if number(e, "period_years").is_none() && number(e, "distance_au").is_none() {
                return Err("a body needs period_years or distance_au".into());
            }
            Ok(())
        }
    }
}

fn validate_star(e: &EntityView<'_>) -> Result<(), String> {
    require_text(e, "name")?;
    match text(e, "role") {
        Some("primary") | Some("companion") => {}
        _ => return Err("role must be primary or companion".into()),
    }
    match number(e, "mass_solar") {
        Some(m) if m.is_finite() && m > 0.0 => {}
        _ => return Err("mass_solar is required and must be a positive number".into()),
    }
    positive(e, "luminosity_solar")?;
    orbit(e)
}

fn validate_calendar(e: &EntityView<'_>) -> Result<(), String> {
    require_text(e, "epoch_name")?;
    match words(e, "weekdays") {
        Some(w) if !w.is_empty() && w.len() <= MAX_WEEKDAYS => {}
        _ => {
            return Err(format!(
                "weekdays is required and must be 1 to {MAX_WEEKDAYS} names"
            ));
        }
    }
    match text(e, "leap_rule") {
        None | Some("none") => Ok(()),
        Some("every_n") => match int(e, "leap_every_n") {
            Some(n) if n >= 2 => optional_entity(e, "leap_month"),
            _ => Err("leap_rule = every_n needs leap_every_n of at least 2".into()),
        },
        _ => Err("leap_rule must be none or every_n".into()),
    }
}

fn validate_month(e: &EntityView<'_>) -> Result<(), String> {
    require_text(e, "name")?;
    require_at_least(e, "index", 1)?;
    require_at_least(e, "days", 1)?;
    require_entity(e, "calendar")
}

fn validate_season(e: &EntityView<'_>) -> Result<(), String> {
    require_text(e, "name")?;
    require_entity(e, "start_month")?;
    require_entity(e, "end_month")?;
    require_at_least(e, "start_day", 1)?;
    require_at_least(e, "end_day", 1)?;
    require_entity(e, "calendar")
}

/// The home world's astronomy fields (§3.3). Every one is optional, and every field this
/// subsystem does not own is left alone — other subsystems write to the planet too.
fn validate_planet(e: &EntityView<'_>) -> Result<(), String> {
    positive(e, "year_days")?;
    positive(e, "day_hours")?;
    positive(e, "radius_km")?;
    positive(e, "gravity_ms2")?;
    range(e, "axial_tilt_deg", 0.0, 90.0)?;
    range(e, "eccentricity", 0.0, 0.95)?;
    at_least(e, "order", 1)?;
    optional_bool(e, "retrograde")?;
    optional_text(e, "name")
}

fn validate(kind: &str, e: &EntityView<'_>) -> Result<(), String> {
    match kind {
        "astro.body" => validate_body(e),
        "astro.star" => validate_star(e),
        "astro.calendar" => validate_calendar(e),
        "astro.month" => validate_month(e),
        "astro.season" => validate_season(e),
        "planet" => validate_planet(e),
        _ => Ok(()),
    }
}

/// Registers a type validator for every kind in [`ASTRO_KINDS`].
pub fn register_kinds(log: &mut EditLog) {
    for &kind in ASTRO_KINDS {
        let k = EntityKind::new(kind).expect("astro kinds are valid keys");
        log.register_validator(k, Box::new(move |e: EntityView<'_>| validate(kind, &e)));
    }
}

/// Adds [`ASTRO_KINDS`] to a Constraints kind catalog, so these entities get verdicts.
pub fn register_catalog(catalog: &mut KindCatalog) {
    for &kind in ASTRO_KINDS {
        catalog.register(kind);
    }
}
