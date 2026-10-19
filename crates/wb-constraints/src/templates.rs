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
        "astro.companion_destabilizes" => {
            "A companion star at {distance_au} AU reaches past {critical_au} AU, destabilizing {count} other bodies ({bodies})."
        }
        "astro.companion_period_mismatch" => {
            "A stated period of {stated_period_years} years and an implied distance of {implied_distance_au} AU disagree with each other."
        }
        "astro.order_mismatch" => {
            "States order {stated}, but sits {places} places out of the distance sequence."
        }
        "astro.orbit_crossing" => {
            "Its orbit ({perihelion_au}–{aphelion_au} AU) crosses {other}'s ({other_perihelion_au}–{other_aphelion_au} AU)."
        }
        "astro.resonance_protects" => {
            "Crosses {other}'s orbit, but a {p}:{q} resonance with it keeps them apart."
        }
        "astro.resonance" => "In a {p}:{q} resonance with {other}.",
        "astro.moon_outside_hill" => {
            "Orbits at {distance_km} km, past a third of its planet's Hill radius ({hill_third_km} km)."
        }
        "astro.moon_inside_roche" => {
            "Orbits at {distance_km} km, inside the Roche limit ({roche_km} km) for its density."
        }
        "astro.belt_overlaps_orbit" => {
            "Spans {inner_au}–{outer_au} AU, which contains {other}'s orbit."
        }
        "astro.outside_habitable_zone" => {
            "An equilibrium temperature of {temperature_k} K ({flux_wm2} W/m² at albedo {albedo}) is outside the habitable band."
        }
        "astro.calendar_sum_mismatch" => {
            "Months sum to {month_total} days against a {year_days}-day year — off by {difference_days} days."
        }
        "astro.leap_drift" => {
            "The leap rule averages {mean_year_days} days a year against a {year_days}-day year, drifting {drift_days} days a year."
        }
        // No placeholder here on purpose: the moon branch and the year/AU branch name
        // this disagreement under different keys (days/km vs. years/AU — Fix 6), so a
        // template that named either would render literally on the other.
        "astro.period_distance_conflict" => {
            "States a period and a distance that are inconsistent with each other by more than 5%."
        }
        "astro.tidal_locking" => {
            "{perturber} would tidally lock this world's day in about {years} years."
        }
        "astro.month_index_duplicated" => {
            "Month index {index} is shared by {months} — dates on it won't round-trip correctly."
        }

        // wb-astro: suggestion codes.
        "astro.move_companion_out" => "move the companion out to {distance_au} AU",
        "astro.demote_companion" => {
            "reduce the companion's mass to {mass_solar} solar masses, below the stellar limit"
        }
        "astro.renumber" => "renumber it to {order}",
        "astro.keep_distance" => "keep the stated distance and drop the stated period",
        "astro.keep_period" => "keep the stated period and drop the stated distance",
        "astro.retune_year" => {
            "retune the year to {year_days} days ({distance_au} AU), accounting for the {basis}"
        }
        "astro.adjust_longest_month" => "adjust {month} to {days} days",
        "astro.add_season" => "add a season named {name} from day {start} to day {end}",
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
