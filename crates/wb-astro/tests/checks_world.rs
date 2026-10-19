mod common;

use common::{add, aethoria, astro_report, codes, earth_system, f, findings, i, named, set, t};
use wb_astro::{SkyModel, equilibrium_temperature_k, locking_time_s, mean_stellar_flux};
use wb_constraints::{FindingKind, Param};
use wb_editlog::{EntityId, Value};

fn close(got: f64, want: f64, rel: f64) {
    let err = (got - want).abs() / want.abs();
    assert!(
        err <= rel,
        "got {got}, want {want} (relative error {err} > {rel})"
    );
}

#[test]
fn earth_gets_earths_flux_and_temperature() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    close(mean_stellar_flux(&sky), 1_361.0, 0.02);
    // The textbook figure: 255 K for an albedo of 0.3.
    close(equilibrium_temperature_k(1_361.0, 0.3), 255.0, 0.01);
    assert_eq!(equilibrium_temperature_k(0.0, 0.3), 0.0);
    assert_eq!(equilibrium_temperature_k(1_361.0, 1.0), 0.0);
    assert!(equilibrium_temperature_k(f64::NAN, 0.3).is_finite());
}

#[test]
fn a_habitable_world_raises_nothing_and_a_frozen_one_does() {
    let r = astro_report(&earth_system());
    assert!(
        findings(&r, "astro.outside_habitable_zone").is_empty(),
        "Earth is habitable: {:?}",
        codes(&r)
    );

    // Push the year out to Jupiter's and the world freezes.
    let mut cold = earth_system();
    set(&mut cold, EntityId::PLANET, vec![("year_days", f(4_333.0))]);
    let r = astro_report(&cold);
    let found = findings(&r, "astro.outside_habitable_zone");
    assert_eq!(found.len(), 1, "{:?}", codes(&r));
    assert_eq!(found[0].badness, 0.6);
    assert!(matches!(found[0].params.get("temperature_k"), Some(Param::Float(t)) if *t < 150.0));
    // The fix retunes the year to land mid-band.
    assert_eq!(found[0].suggestions.len(), 1);
    let fix = found[0].suggestions[0].clone();
    assert_eq!(fix.code.as_str(), "astro.retune_year");
    assert_eq!(fix.writes[0].1, "year_days");
    wb_constraints::apply_suggestion(&mut cold, &fix).unwrap();
    assert!(
        findings(&astro_report(&cold), "astro.outside_habitable_zone").is_empty(),
        "the suggested year should actually be habitable"
    );
}

#[test]
fn a_world_too_close_to_its_star_is_too_hot() {
    let mut log = earth_system();
    set(&mut log, EntityId::PLANET, vec![("year_days", f(60.0))]);
    let r = astro_report(&log);
    let found = findings(&r, "astro.outside_habitable_zone");
    assert_eq!(found.len(), 1);
    assert!(matches!(found[0].params.get("temperature_k"), Some(Param::Float(t)) if *t > 320.0));
}

#[test]
fn a_second_sun_counts_towards_the_temperature() {
    let mut log = earth_system();
    let before = mean_stellar_flux(&SkyModel::build(log.state()).unwrap());
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Warmer")),
            ("role", t("companion")),
            ("mass_solar", f(1.0)),
            ("luminosity_solar", f(1.0)),
            ("distance_au", f(2.0)),
        ],
    );
    let after = mean_stellar_flux(&SkyModel::build(log.state()).unwrap());
    assert!(after > before * 1.1, "{before} then {after}");
}

#[test]
fn aethoria_is_habitable_with_both_its_suns() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    let t = equilibrium_temperature_k(mean_stellar_flux(&sky), sky.home().albedo);
    assert!(
        (250.0..=320.0).contains(&t),
        "Aethoria's equilibrium temperature = {t} K"
    );
    assert!(findings(&astro_report(&aethoria()), "astro.outside_habitable_zone").is_empty());
}

#[test]
fn a_tidally_locked_day_is_a_consequence_not_an_issue() {
    // A heavy moon very close in locks a world fast.
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
    let r = astro_report(&log);
    let found = findings(&r, "astro.tidal_locking");
    assert_eq!(found.len(), 1, "{:?}", codes(&r));
    assert_eq!(found[0].kind, FindingKind::Consequence);
    assert_eq!(found[0].badness, 0.0, "a consequence never costs score");
    assert!(found[0].params.contains_key("years"));
    assert!(found[0].params.contains_key("perturber"));
    // Earth and Aethoria are nowhere near locked.
    assert!(findings(&astro_report(&earth_system()), "astro.tidal_locking").is_empty());
    assert!(findings(&astro_report(&aethoria()), "astro.tidal_locking").is_empty());
}

#[test]
fn the_locking_timescale_is_astronomical_for_earth() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let t = locking_time_s(&sky, 7.342e22, 3.844e8);
    let years = t / (365.2422 * 86_400.0);
    // Earth's spin-down time under the Moon is tens of billions of years.
    assert!(years > 1e9, "Earth would lock in {years} years");
    assert_eq!(locking_time_s(&sky, 0.0, 3.844e8), f64::INFINITY);
    assert_eq!(locking_time_s(&sky, 7.3e22, 0.0), f64::INFINITY);
}

#[test]
fn a_body_stating_both_a_period_and_a_distance_that_disagree_is_reported() {
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
    let r = astro_report(&log);
    let found = findings(&r, "astro.period_distance_conflict");
    assert_eq!(found.len(), 1, "{:?}", codes(&r));
    assert_eq!(found[0].badness, 0.7);
    for key in [
        "stated_period_years",
        "implied_period_years",
        "stated_distance_au",
    ] {
        assert!(found[0].params.contains_key(key), "missing {key}");
    }
    // Two ways to resolve it: keep the distance, or keep the period.
    let codes: Vec<&str> = found[0]
        .suggestions
        .iter()
        .map(|s| s.code.as_str())
        .collect();
    assert!(codes.contains(&"astro.keep_distance"));
    assert!(codes.contains(&"astro.keep_period"));
    let keep_distance = found[0]
        .suggestions
        .iter()
        .find(|s| s.code.as_str() == "astro.keep_distance")
        .unwrap()
        .clone();
    wb_constraints::apply_suggestion(&mut log, &keep_distance).unwrap();
    assert!(findings(&astro_report(&log), "astro.period_distance_conflict").is_empty());
    assert_eq!(
        log.state()
            .field(named(&log, "Contradiction"), "period_years"),
        None,
        "keeping the distance clears the period"
    );
}

#[test]
fn a_body_whose_period_and_distance_agree_is_left_alone() {
    let mut log = earth_system();
    // 4 AU about a solar mass is 8.0 local years.
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Consistent")),
            ("order", i(4)),
            ("class", t("rocky")),
            ("distance_au", f(4.0)),
            ("period_years", f(8.0)),
        ],
    );
    assert!(findings(&astro_report(&log), "astro.period_distance_conflict").is_empty());
}

#[test]
fn months_that_do_not_sum_to_the_year_are_reported_and_fixed() {
    let mut log = aethoria();
    // 13 months of 30 days against a 400-day year: ten days missing.
    set(&mut log, EntityId::PLANET, vec![("year_days", f(400.0))]);
    let r = astro_report(&log);
    let found = findings(&r, "astro.calendar_sum_mismatch");
    assert_eq!(found.len(), 1, "{:?}", codes(&r));
    assert_eq!(found[0].badness, 0.9);
    assert!(
        matches!(found[0].params.get("month_total"), Some(Param::Float(d)) if (*d - 390.0).abs() < 1e-9)
    );
    assert!(
        matches!(found[0].params.get("year_days"), Some(Param::Float(d)) if (*d - 400.0).abs() < 1e-9)
    );
    // The fix lengthens the longest month.
    let fix = found[0].suggestions[0].clone();
    assert_eq!(fix.code.as_str(), "astro.adjust_longest_month");
    assert_eq!(fix.writes[0].1, "days");
    wb_constraints::apply_suggestion(&mut log, &fix).unwrap();
    assert!(findings(&astro_report(&log), "astro.calendar_sum_mismatch").is_empty());
    assert_eq!(
        SkyModel::build(log.state())
            .unwrap()
            .calendar()
            .month_days(),
        400
    );
}

#[test]
fn a_calendar_that_adds_up_raises_nothing() {
    let r = astro_report(&aethoria());
    assert!(
        findings(&r, "astro.calendar_sum_mismatch").is_empty(),
        "{:?}",
        codes(&r)
    );
    assert!(findings(&r, "astro.leap_drift").is_empty());
    // Earth: 365 days plus a leap day every 4 years drifts only 0.0078 days a year.
    let r = astro_report(&earth_system());
    assert!(
        findings(&r, "astro.calendar_sum_mismatch").is_empty(),
        "{:?}",
        codes(&r)
    );
    assert!(findings(&r, "astro.leap_drift").is_empty());
}

#[test]
fn a_leap_rule_that_still_drifts_is_reported() {
    // 365 days with no leap rule at all against a 365.2422-day year: a quarter day a year.
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
    let r = astro_report(&log);
    let found = findings(&r, "astro.leap_drift");
    assert_eq!(found.len(), 1, "{:?}", codes(&r));
    assert_eq!(found[0].badness, 0.6);
    assert!(
        matches!(found[0].params.get("drift_days"), Some(Param::Float(d)) if (*d - 0.2422).abs() < 0.01)
    );
    assert!(
        findings(&r, "astro.calendar_sum_mismatch").is_empty(),
        "a quarter-day drift is drift, not a sum mismatch"
    );
}

#[test]
fn registering_wires_up_kinds_validators_and_checkers_in_one_call() {
    use wb_constraints::{CheckerRegistry, KindCatalog};
    let mut log = wb_editlog::EditLog::new(
        wb_editlog::ActorId([1; 16]),
        wb_editlog::AuthorId([9; 16]),
        Box::new(wb_editlog::FixedClock(1_758_000_000_000)),
    );
    let mut registry = CheckerRegistry::new();
    let mut catalog = KindCatalog::with_starter_kinds();
    wb_astro::register(&mut log, &mut registry, &mut catalog);

    assert_eq!(registry.len(), 9, "all nine checkers: {:?}", registry.ids());
    for id in [
        "astro.belt",
        "astro.body",
        "astro.calendar",
        "astro.climate",
        "astro.companion",
        "astro.moons",
        "astro.orbits",
        "astro.ordering",
        "astro.spin",
    ] {
        assert!(registry.ids().contains(&id), "{id} not registered");
    }
    for k in wb_astro::ASTRO_KINDS {
        assert!(catalog.contains(k));
    }
    // The validators are live: an invalid body is rejected.
    let mut tx = log.transact("bad");
    tx.create("astro.body", vec![("name", t("X"))]);
    assert!(tx.commit().is_err());
}
