//! Display text for the codes this subsystem owns.
//!
//! These strings belong here, not in `wb-constraints`' own table: a subsystem's copy is
//! the subsystem's business, and the next one along would otherwise pile onto a shared
//! string table it has no stake in. `wb-constraints` keeps its v1 rows and the fallback
//! that renders an unregistered code as `code key=value`.
//!
//! Every `{placeholder}` must name a param the corresponding finding actually carries —
//! one that does not renders as literal braces, which is worse than the fallback. The
//! test in `tests/templates.rs` holds that line.

use wb_constraints::TemplateRegistry;

/// Every code this subsystem renders, as `(code, template)`.
pub const ASTRO_TEMPLATES: &[(&str, &str)] = &[
    (
        "astro.companion_destabilizes",
        "A companion star at {distance_au} AU leaves orbits between {critical_au} and {outer_critical_au} AU unstable; {count} bodies sit in that band ({bodies}).",
    ),
    (
        "astro.companion_period_mismatch",
        "A stated period of {stated_period_years} years and an implied distance of {implied_distance_au} AU disagree with each other.",
    ),
    (
        "astro.order_mismatch",
        "States order {stated}, but sits {places} places out of the distance sequence.",
    ),
    (
        "astro.orbit_crossing",
        "Its orbit ({perihelion_au}–{aphelion_au} AU) crosses {other}'s ({other_perihelion_au}–{other_aphelion_au} AU).",
    ),
    (
        "astro.resonance_protects",
        "Crosses {other}'s orbit, but a {p}:{q} resonance with it keeps them apart.",
    ),
    ("astro.resonance", "In a {p}:{q} resonance with {other}."),
    (
        "astro.moon_outside_hill",
        "Orbits at {distance_km} km, past a third of its planet's Hill radius ({hill_third_km} km).",
    ),
    (
        "astro.moon_inside_roche",
        "Orbits at {distance_km} km, inside the Roche limit ({roche_km} km) for its density.",
    ),
    (
        "astro.belt_overlaps_orbit",
        "Spans {inner_au}–{outer_au} AU, which contains {other}'s orbit.",
    ),
    (
        "astro.outside_habitable_zone",
        "An equilibrium temperature of {temperature_k} K ({flux_wm2} W/m² at albedo {albedo}) is outside the habitable band.",
    ),
    (
        "astro.calendar_sum_mismatch",
        "Months sum to {month_total} days against a {year_days}-day year — off by {difference_days} days.",
    ),
    (
        "astro.leap_drift",
        "The leap rule averages {mean_year_days} days a year against a {year_days}-day year, drifting {drift_days} days a year.",
    ),
    (
        "astro.period_distance_conflict",
        "States a period and a distance that are inconsistent with each other by more than 5%.",
    ),
    (
        "astro.tidal_locking",
        "{perturber} would tidally lock this world's day in about {years} years.",
    ),
    (
        "astro.month_index_duplicated",
        "Month index {index} is shared by {months} — dates on it won't round-trip correctly.",
    ),
    (
        "astro.move_companion_out",
        "move the companion out to {distance_au} AU",
    ),
    (
        "astro.demote_companion",
        "reduce the companion's mass to {mass_solar} solar masses, below the stellar limit",
    ),
    ("astro.renumber", "renumber it to {order}"),
    (
        "astro.keep_distance",
        "keep the stated distance and drop the stated period",
    ),
    (
        "astro.keep_period",
        "keep the stated period and drop the stated distance",
    ),
    (
        "astro.retune_year",
        "retune the year to {year_days} days ({distance_au} AU), accounting for the {basis}",
    ),
    (
        "astro.adjust_longest_month",
        "adjust {month} to {days} days",
    ),
    (
        "astro.add_season",
        "add a season named {name} from day {start} to day {end}",
    ),
];

/// Registers this subsystem's display text.
pub fn register_templates(registry: &mut TemplateRegistry) {
    for (code, template) in ASTRO_TEMPLATES {
        registry.register(code, template);
    }
}
