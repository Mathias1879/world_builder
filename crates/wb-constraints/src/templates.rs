use crate::model::{Finding, Param, Suggestion};
use std::collections::BTreeMap;

fn template(code: &str) -> Option<&'static str> {
    Some(match code {
        "coast.over_ocean" => "{pct}% of this feature lies over the ocean.",
        "lore.area_mismatch" => {
            "Drawn area is {drawn} {unit} but the lore says {stated} {unit} ({ratio}× off)."
        }
        "lore.area_consistent" => "Drawn area matches the lore ({drawn} {unit}).",
        "lore.set_area_to_drawn" => "set the stated area to {drawn} {unit}",
        "lore.relative_position_violated" => {
            "The lore says this lies {relation} its reference, but it is {degrees}° the wrong way."
        }
        "lore.relative_position_ok" => "Placement matches the lore ({relation}).",
        "rule.forbidden_feature" => "A {feature_kind} lies inside a {rule} region.",
        "river.inland_mouth" => "This river ends inland without a lake.",
        "river.source_in_ocean" => "This river's source is in the ocean.",
        "climate.lowland_tropical_glacier" => {
            "A lowland glacier at {lat}° latitude is unlikely without high elevation."
        }
        "climate.tropical_forest_high_latitude" => {
            "Tropical forest at {lat}° latitude is unlikely."
        }
        "climate.boreal_forest_low_latitude" => "Boreal forest at {lat}° latitude is unlikely.",

        // wb-astro: issue codes (spec §6). A per-subsystem template registry is the
        // properly layered home for these (recorded as a follow-up); for now they live
        // here, which is better than no explanation at all.
        _ => return None,
    })
}

fn fmt_param(p: &Param) -> String {
    match p {
        Param::Int(i) => i.to_string(),
        Param::Float(x) if x.abs() >= 100.0 => format!("{x:.0}"),
        Param::Float(x) => format!("{x:.2}"),
        Param::Text(s) => s.clone(),
        Param::Entity(e) => format!("#{}.{}", e.op.lamport, e.n),
    }
}

fn fill(code: &str, params: &BTreeMap<String, Param>) -> String {
    fill_with(template(code), code, params)
}
/// Display templates a subsystem registers for the codes it owns.
///
/// [`template`] holds the v1 catalog's own rows, and an unregistered code still renders
/// as `code key=value` rather than failing — but a subsystem's copy belongs to that
/// subsystem, not to this string table. `wb-astro` registers its own; Pipeline and the
/// rest will do the same. Same shape as [`crate::KindCatalog`]: starter behaviour stays
/// exactly as it was, and the registry is the additive extension point.
#[derive(Clone, Debug, Default)]
pub struct TemplateRegistry {
    templates: BTreeMap<String, &'static str>,
}

impl TemplateRegistry {
    /// An empty registry. Lookups still fall back to the starter catalog's rows, so this
    /// renders every v1 code correctly on its own.
    pub fn new() -> Self {
        TemplateRegistry::default()
    }

    /// Registers one code's template. Registering a code twice keeps the last one.
    pub fn register(&mut self, code: &str, template: &'static str) {
        self.templates.insert(code.to_string(), template);
    }

    /// The template for a code: a registered one, else the starter catalog's.
    pub fn get(&self, code: &str) -> Option<&'static str> {
        self.templates.get(code).copied().or_else(|| template(code))
    }

    pub fn len(&self) -> usize {
        self.templates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }
}

/// Fills a template, or falls back to `code key=value` when there is none.
fn fill_with(t: Option<&'static str>, code: &str, params: &BTreeMap<String, Param>) -> String {
    match t {
        Some(t) => {
            let mut out = t.to_string();
            for (k, v) in params {
                out = out.replace(&format!("{{{k}}}"), &fmt_param(v));
            }
            out
        }
        None => {
            let mut out = code.to_string();
            for (k, v) in params {
                out.push_str(&format!(" {k}={}", fmt_param(v)));
            }
            out
        }
    }
}

/// Human-readable explanation of a finding, using `registry`'s templates.
pub fn render_with(registry: &TemplateRegistry, f: &Finding) -> String {
    fill_with(registry.get(f.code.as_str()), f.code.as_str(), &f.params)
}

/// Human-readable description of a suggestion, using `registry`'s templates.
pub fn render_suggestion_with(registry: &TemplateRegistry, s: &Suggestion) -> String {
    fill_with(registry.get(s.code.as_str()), s.code.as_str(), &s.params)
}

/// Human-readable explanation of a finding.
pub fn render(f: &Finding) -> String {
    fill(f.code.as_str(), &f.params)
}

/// Human-readable description of a suggestion.
pub fn render_suggestion(s: &Suggestion) -> String {
    fill(s.code.as_str(), &s.params)
}
