//! Every astro finding and suggestion renders real words, not a debug string.
//!
//! This subsystem registers a template for every issue and suggestion code it owns, so
//! this walks a handful of fixtures known
//! to raise each one and checks the rendered text for two failure signs: a `{param}`
//! placeholder that never got filled (meaning the template referenced a key absent from
//! that finding's params), and the unrendered fallback format itself (`code key=value`),
//! which is what every astro finding looked like before Fix 1.

mod common;

use common::{add, aethoria, astro_report, earth_system, f, i, named, set, t};
use wb_astro::SkyModel;
use wb_constraints::{
    Finding, Report, Suggestion, TemplateRegistry, render_suggestion_with, render_with,
};
use wb_editlog::{EntityId, Value};

/// A registry holding this subsystem's rows — what a real consumer renders through.
fn astro_templates() -> TemplateRegistry {
    let mut r = TemplateRegistry::new();
    wb_astro::register_templates(&mut r);
    r
}

/// Fails loudly, naming the code and the raw text, rather than a bare `assert!`.
fn assert_renders_cleanly(code: &str, text: &str) {
    assert!(
        !text.contains('{') && !text.contains('}'),
        "{code} left an unfilled placeholder: {text:?}"
    );
    assert!(
        !text.starts_with(code),
        "{code} rendered as the unrendered fallback, not a template: {text:?}"
    );
}

fn check_report(r: &Report) {
    for v in &r.verdicts {
        for f in v
            .issues
            .iter()
            .chain(&v.intentional)
            .chain(&v.supports)
            .chain(&v.consequences)
        {
            check_finding(f);
        }
    }
}

fn check_finding(f: &Finding) {
    assert_renders_cleanly(f.code.as_str(), &render_with(&astro_templates(), f));
    for s in &f.suggestions {
        check_suggestion(s);
    }
}

fn check_suggestion(s: &Suggestion) {
    assert_renders_cleanly(
        s.code.as_str(),
        &render_suggestion_with(&astro_templates(), s),
    );
}

#[test]
fn every_astro_issue_and_support_code_renders_real_words() {
    // Aethoria alone covers the companion, resonance and tidal/calendar-free cases.
    check_report(&astro_report(&aethoria()));

    // astro.companion_destabilizes, astro.move_companion_out, astro.demote_companion.
    let mut log = earth_system();
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Intruder")),
            ("role", t("companion")),
            ("mass_solar", f(0.8)),
            ("distance_au", f(2.0)),
        ],
    );
    check_report(&astro_report(&log));

    // astro.companion_period_mismatch (both stated), full params.
    let mut log = earth_system();
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Secundus")),
            ("role", t("companion")),
            ("mass_solar", f(0.33)),
            ("distance_au", f(40.0)),
            ("period_years", f(1.0)),
        ],
    );
    check_report(&astro_report(&log));

    // astro.companion_period_mismatch (period only), the trimmed-params branch.
    let mut log = earth_system();
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("PeriodOnly")),
            ("role", t("companion")),
            ("mass_solar", f(0.2)),
            ("order", i(9)),
            ("period_years", f(5.0)),
        ],
    );
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Outer")),
            ("order", i(10)),
            ("class", t("rocky")),
            ("distance_au", f(50.0)),
        ],
    );
    check_report(&astro_report(&log));

    // astro.order_mismatch, astro.renumber.
    let mut log = earth_system();
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Near")),
            ("order", i(9)),
            ("class", t("rocky")),
            ("distance_au", f(2.0)),
        ],
    );
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Far")),
            ("order", i(2)),
            ("class", t("rocky")),
            ("distance_au", f(20.0)),
        ],
    );
    check_report(&astro_report(&log));

    // astro.orbit_crossing.
    let mut log = earth_system();
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Inner")),
            ("order", i(4)),
            ("class", t("rocky")),
            ("distance_au", f(3.0)),
            ("eccentricity", f(0.5)),
        ],
    );
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Outer")),
            ("order", i(5)),
            ("class", t("rocky")),
            ("distance_au", f(4.1)),
            ("eccentricity", f(0.3)),
        ],
    );
    check_report(&astro_report(&log));

    // astro.resonance (support, no crossing).
    let mut log = earth_system();
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("A")),
            ("order", i(4)),
            ("class", t("rocky")),
            ("distance_au", f(4.0)),
        ],
    );
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("B")),
            ("order", i(5)),
            ("class", t("rocky")),
            ("distance_au", f(4.0 * 1.587_401)),
        ],
    );
    check_report(&astro_report(&log));

    // astro.resonance_protects (Neptune/Pluto's 3:2).
    let mut log = earth_system();
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Neptune")),
            ("order", i(8)),
            ("class", t("ice_giant")),
            ("distance_au", f(30.07)),
        ],
    );
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Pluto")),
            ("order", i(9)),
            ("class", t("rocky")),
            ("distance_au", f(39.48)),
            ("eccentricity", f(0.2488)),
        ],
    );
    check_report(&astro_report(&log));

    // astro.moon_outside_hill.
    let mut log = earth_system();
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Runaway")),
            ("order", i(2)),
            ("class", t("moon")),
            ("parent", Value::Entity(EntityId::PLANET)),
            ("distance_km", f(900_000.0)),
        ],
    );
    check_report(&astro_report(&log));

    // astro.moon_inside_roche.
    let mut log = earth_system();
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Doomed")),
            ("order", i(2)),
            ("class", t("moon")),
            ("parent", Value::Entity(EntityId::PLANET)),
            ("distance_km", f(8_000.0)),
            ("density_kgm3", f(3_344.0)),
        ],
    );
    check_report(&astro_report(&log));

    // astro.belt_overlaps_orbit.
    let mut log = earth_system();
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Wide Belt")),
            ("order", i(4)),
            ("class", t("belt")),
            ("inner_au", f(2.0)),
            ("outer_au", f(6.0)),
        ],
    );
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Resident")),
            ("order", i(5)),
            ("class", t("gas_giant")),
            ("distance_au", f(4.0)),
        ],
    );
    check_report(&astro_report(&log));

    // astro.outside_habitable_zone, astro.retune_year.
    let mut cold = earth_system();
    set(&mut cold, EntityId::PLANET, vec![("year_days", f(4_333.0))]);
    check_report(&astro_report(&cold));

    // astro.calendar_sum_mismatch, astro.adjust_longest_month.
    let mut log = aethoria();
    set(&mut log, EntityId::PLANET, vec![("year_days", f(400.0))]);
    check_report(&astro_report(&log));

    // astro.leap_drift.
    let mut log = common::new_log();
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("S")),
            ("role", t("primary")),
            ("mass_solar", f(1.0)),
        ],
    );
    let cal = add(
        &mut log,
        "astro.calendar",
        vec![("epoch_name", t("CE")), ("weekdays", common::list(&["A"]))],
    );
    add(
        &mut log,
        "astro.month",
        vec![
            ("index", i(1)),
            ("name", t("Long")),
            ("days", i(365)),
            ("calendar", Value::Entity(cal)),
        ],
    );
    check_report(&astro_report(&log));

    // astro.period_distance_conflict (year/AU branch), astro.keep_distance/keep_period.
    let mut log = earth_system();
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Contradiction")),
            ("order", i(4)),
            ("class", t("rocky")),
            ("distance_au", f(4.0)),
            ("period_years", f(2.0)),
        ],
    );
    check_report(&astro_report(&log));

    // astro.period_distance_conflict (moon, days/km branch).
    let mut log = earth_system();
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("MoonContradiction")),
            ("order", i(2)),
            ("class", t("moon")),
            ("parent", Value::Entity(EntityId::PLANET)),
            ("distance_km", f(300_000.0)),
            ("period_days", f(1_000.0)),
        ],
    );
    check_report(&astro_report(&log));

    // astro.tidal_locking (consequence).
    let mut log = earth_system();
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Anvil")),
            ("order", i(2)),
            ("class", t("moon")),
            ("parent", Value::Entity(EntityId::PLANET)),
            ("distance_km", f(30_000.0)),
            ("radius_km", f(3_000.0)),
            ("density_kgm3", f(5_000.0)),
        ],
    );
    check_report(&astro_report(&log));

    // astro.month_index_duplicated.
    let mut log = aethoria();
    let solspire = named(&log, "Solspire");
    set(&mut log, solspire, vec![("index", i(1))]);
    check_report(&astro_report(&log));

    // astro.add_season, straight from propose_seasons rather than a finding.
    let sky = SkyModel::build(aethoria().state()).unwrap();
    for s in sky.propose_seasons(4) {
        check_suggestion(&s);
    }
}
