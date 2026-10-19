use std::collections::BTreeSet;
use wb_editlog::{EditLog, EntityId, EntityKind, EntityView, Value};
use wb_grid::LatLon;
use wb_world::Geometry;

/// The set of entity kinds Constraints judges.
///
/// [`crate::evaluate`] visits a live entity only when its kind is in the catalog carried
/// by [`crate::CheckContext`], so each subsystem registers the kinds it owns and nothing
/// else needs a central list. `with_starter_kinds` seeds the v1 catalog
/// ([`CONSTRAINT_KINDS`]); `wb-astro` and later subsystems `register` onto it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KindCatalog {
    kinds: BTreeSet<String>,
}

impl KindCatalog {
    pub fn new() -> Self {
        KindCatalog::default()
    }

    pub fn with_starter_kinds() -> Self {
        let mut c = KindCatalog::new();
        for &k in CONSTRAINT_KINDS {
            c.register(k);
        }
        c
    }

    /// Adds a kind. Registering the same kind twice is harmless.
    pub fn register(&mut self, kind: &str) {
        self.kinds.insert(kind.to_string());
    }

    pub fn contains(&self, kind: &str) -> bool {
        self.kinds.contains(kind)
    }

    /// Registered kinds in sorted order.
    pub fn kinds(&self) -> impl Iterator<Item = &str> {
        self.kinds.iter().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }
}

/// The v1 constraint-kind catalog.
///
/// This is no longer the only gate on what [`crate::evaluate`] looks at: that gate is
/// [`KindCatalog`], carried in [`crate::CheckContext`], and this array is only the
/// starter set it seeds via `KindCatalog::with_starter_kinds`. A subsystem that wants its
/// own entities judged — `wb-astro` is the first — registers its kinds onto the catalog
/// instead of editing this array, which is why the catalog, not this list, is where
/// "does Constraints see this kind at all" is actually decided.
///
/// An entity whose kind is in neither the starter set nor anything a subsystem
/// registered is still silently ignored by `evaluate`: no verdict, no error, no warning.
/// That remains the wanted behaviour for non-constraint entities (`import.base_map` and
/// friends), but it also means a typo'd or forgotten `catalog.register` call makes a kind
/// disappear from the report with nothing to say why.
pub const CONSTRAINT_KINDS: &[&str] = &[
    "feature.mountain_range",
    "feature.hills",
    "feature.forest",
    "feature.desert",
    "feature.lake",
    "feature.glacier",
    "feature.river",
    "civ.settlement",
    "rule.region",
    "lore.area",
    "lore.relative_position",
];

/// Whether `kind` is in the **starter** catalog.
///
/// Kept for callers that have no [`CheckContext`]; [`crate::evaluate`] uses the context's
/// [`KindCatalog`], which may hold more.
pub fn is_constraint_kind(kind: &str) -> bool {
    CONSTRAINT_KINDS.contains(&kind)
}

pub fn polygon<'a>(e: &EntityView<'a>, field: &str) -> Option<&'a [LatLon]> {
    match e.get(field) {
        Some(Value::Geometry(Geometry::Polygon(r))) => Some(r.as_slice()),
        _ => None,
    }
}

pub fn line<'a>(e: &EntityView<'a>, field: &str) -> Option<&'a [LatLon]> {
    match e.get(field) {
        Some(Value::Geometry(Geometry::LineString(l))) => Some(l.as_slice()),
        _ => None,
    }
}

pub fn point(e: &EntityView<'_>, field: &str) -> Option<LatLon> {
    match e.get(field) {
        Some(Value::LatLon(p)) => Some(*p),
        _ => None,
    }
}

pub fn text<'a>(e: &EntityView<'a>, field: &str) -> Option<&'a str> {
    match e.get(field) {
        Some(Value::Text(s)) => Some(s.as_str()),
        _ => None,
    }
}

pub fn float(e: &EntityView<'_>, field: &str) -> Option<f64> {
    match e.get(field) {
        Some(Value::Float(x)) => Some(*x),
        _ => None,
    }
}

pub fn entity(e: &EntityView<'_>, field: &str) -> Option<EntityId> {
    match e.get(field) {
        Some(Value::Entity(id)) => Some(*id),
        _ => None,
    }
}

/// The entity's geometry as points: `area` (closed), else `spine`/`path`, else `at`.
pub fn shape(e: &EntityView<'_>) -> Option<(Vec<LatLon>, bool)> {
    if let Some(r) = polygon(e, "area") {
        return Some((r.to_vec(), true));
    }
    if let Some(l) = line(e, "spine").or_else(|| line(e, "path")) {
        return Some((l.to_vec(), false));
    }
    point(e, "at").map(|p| (vec![p], false))
}

fn one_of(e: &EntityView<'_>, field: &str, allowed: &[&str], required: bool) -> Result<(), String> {
    match e.get(field) {
        None if !required => Ok(()),
        Some(Value::Text(s)) if allowed.contains(&s.as_str()) => Ok(()),
        _ => Err(format!("{field} must be one of {allowed:?}")),
    }
}

fn need<T>(value: Option<T>, what: &str) -> Result<(), String> {
    value
        .map(|_| ())
        .ok_or_else(|| format!("{what} is required"))
}

fn optional_float(e: &EntityView<'_>, field: &str) -> Result<(), String> {
    match e.get(field) {
        None | Some(Value::Float(_)) => Ok(()),
        _ => Err(format!("{field} must be a number")),
    }
}

fn optional_text(e: &EntityView<'_>, field: &str) -> Result<(), String> {
    match e.get(field) {
        None | Some(Value::Text(_)) => Ok(()),
        _ => Err(format!("{field} must be text")),
    }
}

fn common(e: &EntityView<'_>) -> Result<(), String> {
    one_of(e, "strength", &["target", "hard"], false)?;
    match e.get("tolerance") {
        None => {}
        Some(Value::Float(t)) if (-1.0..=1.0).contains(t) => {}
        _ => return Err("tolerance must be a number in [-1, 1]".into()),
    }
    match e.get("keep_codes") {
        None => {}
        Some(Value::List(items)) if items.iter().all(|v| matches!(v, Value::Text(_))) => {}
        _ => return Err("keep_codes must be a list of text".into()),
    }
    for f in ["name", "keep_reason"] {
        match e.get(f) {
            None | Some(Value::Text(_)) => {}
            _ => return Err(format!("{f} must be text")),
        }
    }
    Ok(())
}

fn validate(kind: &str, e: &EntityView<'_>) -> Result<(), String> {
    common(e)?;
    match kind {
        "feature.mountain_range" | "feature.hills" => {
            need(
                line(e, "spine").or_else(|| polygon(e, "area")),
                "spine or area",
            )?;
            optional_float(e, "peak_m")
        }
        "feature.forest" => {
            need(polygon(e, "area"), "area")?;
            one_of(e, "biome", &["tropical", "temperate", "boreal"], false)
        }
        "feature.desert" | "feature.lake" => need(polygon(e, "area"), "area"),
        "feature.glacier" => {
            need(polygon(e, "area"), "area")?;
            optional_float(e, "elevation_m")
        }
        "feature.river" => match line(e, "path") {
            Some(p) if p.len() >= 2 => Ok(()),
            _ => Err("path must be a line with at least 2 points".into()),
        },
        "civ.settlement" => {
            need(point(e, "at"), "at")?;
            need(text(e, "name"), "name")?;
            optional_text(e, "role")
        }
        "rule.region" => {
            need(polygon(e, "area"), "area")?;
            one_of(
                e,
                "rule",
                &["no_rain", "permanent_ice", "perpetual_storm"],
                true,
            )
        }
        "lore.area" => {
            need(entity(e, "subject"), "subject")?;
            match float(e, "value") {
                Some(v) if v > 0.0 => {}
                _ => return Err("value must be a positive number".into()),
            }
            one_of(e, "unit", &["km2", "mi2"], true)
        }
        "lore.relative_position" => {
            need(entity(e, "subject"), "subject")?;
            need(entity(e, "object"), "object")?;
            one_of(
                e,
                "relation",
                &["north_of", "south_of", "east_of", "west_of"],
                true,
            )
        }
        _ => Ok(()),
    }
}

/// Registers type/enumeration validators for every catalog kind.
pub fn register_kinds(log: &mut EditLog) {
    for &kind in CONSTRAINT_KINDS {
        let k = EntityKind::new(kind).expect("catalog kinds are valid keys");
        log.register_validator(k, Box::new(move |e: EntityView<'_>| validate(kind, &e)));
    }
}
