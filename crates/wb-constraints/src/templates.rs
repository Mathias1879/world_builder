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
    match template(code) {
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

/// Human-readable explanation of a finding.
pub fn render(f: &Finding) -> String {
    fill(f.code.as_str(), &f.params)
}

/// Human-readable description of a suggestion.
pub fn render_suggestion(s: &Suggestion) -> String {
    fill(s.code.as_str(), &s.params)
}
