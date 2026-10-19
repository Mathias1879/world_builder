mod common;

use common::{add, f, i, list, new_log, set, t, try_add};
use wb_astro::{ASTRO_KINDS, register_catalog};
use wb_constraints::KindCatalog;
use wb_editlog::{EntityId, Value};

#[test]
fn the_catalog_gains_every_astro_kind() {
    let mut c = KindCatalog::with_starter_kinds();
    let before = c.len();
    register_catalog(&mut c);
    // None of ASTRO_KINDS collides with CONSTRAINT_KINDS (verified by inspecting
    // c.kinds()), so every one of the six is genuinely new to the starter catalog.
    assert_eq!(
        c.len(),
        before + ASTRO_KINDS.len(),
        "all six astro kinds are new"
    );
    for &k in ASTRO_KINDS {
        assert!(c.contains(k), "{k} not registered");
    }
    assert!(c.contains("feature.river"), "starter kinds survive");
}

#[test]
fn a_well_formed_system_is_accepted() {
    let mut log = new_log();
    let star = add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Sol Prime")),
            ("role", t("primary")),
            ("mass_solar", f(1.0)),
        ],
    );
    let planet = add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Pyrox")),
            ("order", i(1)),
            ("class", t("rocky")),
            ("distance_au", f(0.4)),
        ],
    );
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Ashlet")),
            ("order", i(1)),
            ("class", t("moon")),
            ("parent", Value::Entity(planet)),
            ("period_days", f(12.0)),
        ],
    );
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("The Veil")),
            ("order", i(7)),
            ("class", t("belt")),
            ("inner_au", f(3.0)),
            ("outer_au", f(4.0)),
        ],
    );
    let cal = add(
        &mut log,
        "astro.calendar",
        vec![
            ("epoch_name", t("AE")),
            ("weekdays", list(&["Solday", "Lumaday"])),
        ],
    );
    let m1 = add(
        &mut log,
        "astro.month",
        vec![
            ("index", i(1)),
            ("name", t("Mehr")),
            ("days", i(30)),
            ("calendar", Value::Entity(cal)),
        ],
    );
    add(
        &mut log,
        "astro.season",
        vec![
            ("name", t("Primary Summer")),
            ("start_month", Value::Entity(m1)),
            ("start_day", i(1)),
            ("end_month", Value::Entity(m1)),
            ("end_day", i(30)),
            ("calendar", Value::Entity(cal)),
        ],
    );
    set(
        &mut log,
        EntityId::PLANET,
        vec![
            ("order", i(5)),
            ("year_days", f(390.0)),
            ("day_hours", f(30.0)),
            ("axial_tilt_deg", f(23.0)),
        ],
    );
    assert!(star != planet);
}

#[test]
fn a_body_needs_a_name_an_order_and_a_class() {
    let mut log = new_log();
    for fields in [
        vec![
            ("order", i(1)),
            ("class", t("rocky")),
            ("distance_au", f(1.0)),
        ],
        vec![
            ("name", t("X")),
            ("class", t("rocky")),
            ("distance_au", f(1.0)),
        ],
        vec![("name", t("X")), ("order", i(1)), ("distance_au", f(1.0))],
    ] {
        assert!(
            try_add(&mut log, "astro.body", fields).is_err(),
            "a body missing a required field was accepted"
        );
    }
}

#[test]
fn a_body_needs_a_period_or_a_distance() {
    let mut log = new_log();
    let bare = vec![("name", t("X")), ("order", i(1)), ("class", t("rocky"))];
    assert!(try_add(&mut log, "astro.body", bare).is_err());
    // Either one alone is fine; both together is a feasibility finding, not an error.
    assert!(
        try_add(
            &mut log,
            "astro.body",
            vec![
                ("name", t("X")),
                ("order", i(1)),
                ("class", t("rocky")),
                ("period_years", f(1.0)),
                ("distance_au", f(1.0)),
            ],
        )
        .is_ok()
    );
}

#[test]
fn a_moon_uses_days_and_kilometres() {
    let mut log = new_log();
    let p = add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Host")),
            ("order", i(1)),
            ("class", t("rocky")),
            ("distance_au", f(1.0)),
        ],
    );
    assert!(
        try_add(
            &mut log,
            "astro.body",
            vec![
                ("name", t("M")),
                ("order", i(1)),
                ("class", t("moon")),
                ("parent", Value::Entity(p)),
                ("distance_km", f(400_000.0)),
            ],
        )
        .is_ok()
    );
    // A moon with only a year-scale period has nothing to orbit by.
    assert!(
        try_add(
            &mut log,
            "astro.body",
            vec![
                ("name", t("M")),
                ("order", i(1)),
                ("class", t("moon")),
                ("parent", Value::Entity(p)),
            ],
        )
        .is_err()
    );
}

#[test]
fn a_belt_needs_an_extent() {
    let mut log = new_log();
    let belt = |extent: Vec<(&'static str, Value)>| {
        let mut v = vec![("name", t("Veil")), ("order", i(7)), ("class", t("belt"))];
        v.extend(extent);
        v
    };
    assert!(try_add(&mut log, "astro.body", belt(vec![])).is_err());
    assert!(try_add(&mut log, "astro.body", belt(vec![("inner_au", f(3.0))])).is_err());
    assert!(
        try_add(
            &mut log,
            "astro.body",
            belt(vec![("inner_au", f(3.0)), ("outer_au", f(4.0))])
        )
        .is_ok()
    );
}

#[test]
fn out_of_range_numbers_are_rejected() {
    let mut log = new_log();
    let body = |extra: (&'static str, Value)| {
        vec![
            ("name", t("X")),
            ("order", i(1)),
            ("class", t("rocky")),
            ("distance_au", f(1.0)),
            extra,
        ]
    };
    for bad in [
        ("eccentricity", f(1.0)),
        ("eccentricity", f(-0.1)),
        ("inclination_deg", f(91.0)),
        ("inclination_deg", f(-91.0)),
        ("radius_km", f(0.0)),
        ("density_kgm3", f(-1.0)),
        ("albedo", f(1.5)),
        ("distance_au", f(0.0)),
        ("order", i(0)),
        ("class", t("dyson_sphere")),
        ("name", f(3.0)),
        ("rings", f(1.0)),
    ] {
        assert!(
            try_add(&mut log, "astro.body", body(bad.clone())).is_err(),
            "{bad:?} was accepted"
        );
    }
    assert!(try_add(&mut log, "astro.body", body(("rings", Value::Bool(true)))).is_ok());
}

#[test]
fn a_star_needs_a_role_and_a_mass() {
    let mut log = new_log();
    assert!(
        try_add(
            &mut log,
            "astro.star",
            vec![
                ("name", t("S")),
                ("role", t("secondary")),
                ("mass_solar", f(1.0))
            ],
        )
        .is_err(),
        "role is primary | companion"
    );
    assert!(
        try_add(
            &mut log,
            "astro.star",
            vec![("name", t("S")), ("role", t("companion"))],
        )
        .is_err(),
        "mass_solar is required"
    );
    assert!(
        try_add(
            &mut log,
            "astro.star",
            vec![
                ("name", t("S")),
                ("role", t("companion")),
                ("mass_solar", f(0.33)),
                ("luminosity_solar", f(0.02)),
                ("distance_au", f(12.0)),
                ("order", i(8)),
            ],
        )
        .is_ok()
    );
    assert!(
        try_add(
            &mut log,
            "astro.star",
            vec![
                ("name", t("S")),
                ("role", t("primary")),
                ("mass_solar", f(0.0))
            ],
        )
        .is_err()
    );
    assert!(
        try_add(
            &mut log,
            "astro.star",
            vec![
                ("name", t("S")),
                ("role", t("primary")),
                ("mass_solar", f(1.0)),
                ("luminosity_solar", f(0.0)),
            ],
        )
        .is_err(),
        "a star cannot radiate zero or less"
    );
    assert!(
        try_add(
            &mut log,
            "astro.star",
            vec![
                ("name", t("S")),
                ("role", t("primary")),
                ("mass_solar", f(1.0)),
                ("luminosity_solar", f(-1.0)),
            ],
        )
        .is_err(),
        "a star cannot radiate zero or less"
    );
}

#[test]
fn calendar_entities_are_checked() {
    let mut log = new_log();
    // weekdays must be a non-empty list of text, at most 100 long.
    assert!(
        try_add(
            &mut log,
            "astro.calendar",
            vec![("epoch_name", t("AE")), ("weekdays", Value::List(vec![]))],
        )
        .is_err()
    );
    assert!(
        try_add(
            &mut log,
            "astro.calendar",
            vec![
                ("epoch_name", t("AE")),
                ("weekdays", Value::List(vec![f(1.0)])),
            ],
        )
        .is_err()
    );
    let long: Vec<&str> = (0..101).map(|_| "Day").collect();
    assert!(
        try_add(
            &mut log,
            "astro.calendar",
            vec![("epoch_name", t("AE")), ("weekdays", list(&long))],
        )
        .is_err()
    );
    // every_n needs its period.
    assert!(
        try_add(
            &mut log,
            "astro.calendar",
            vec![
                ("epoch_name", t("AE")),
                ("weekdays", list(&["A"])),
                ("leap_rule", t("every_n")),
            ],
        )
        .is_err()
    );
    let cal = add(
        &mut log,
        "astro.calendar",
        vec![
            ("epoch_name", t("AE")),
            ("weekdays", list(&["A"])),
            ("leap_rule", t("every_n")),
            ("leap_every_n", i(4)),
        ],
    );
    // A month needs an index, a name, positive days and its calendar.
    assert!(
        try_add(
            &mut log,
            "astro.month",
            vec![
                ("index", i(1)),
                ("name", t("M")),
                ("days", i(0)),
                ("calendar", Value::Entity(cal))
            ],
        )
        .is_err()
    );
    assert!(
        try_add(
            &mut log,
            "astro.month",
            vec![("index", i(1)), ("name", t("M")), ("days", i(30))],
        )
        .is_err(),
        "calendar is required"
    );
    let m = add(
        &mut log,
        "astro.month",
        vec![
            ("index", i(1)),
            ("name", t("M")),
            ("days", i(30)),
            ("calendar", Value::Entity(cal)),
        ],
    );
    // A season needs both ends.
    assert!(
        try_add(
            &mut log,
            "astro.season",
            vec![
                ("name", t("S")),
                ("start_month", Value::Entity(m)),
                ("start_day", i(1)),
                ("calendar", Value::Entity(cal)),
            ],
        )
        .is_err()
    );
    assert!(
        try_add(
            &mut log,
            "astro.season",
            vec![
                ("name", t("S")),
                ("start_month", Value::Entity(m)),
                ("start_day", i(1)),
                ("end_month", Value::Entity(m)),
                ("end_day", i(30)),
                ("calendar", Value::Entity(cal)),
            ],
        )
        .is_ok()
    );
}

#[test]
fn the_home_world_fields_are_checked_but_optional() {
    let mut log = new_log();
    // The planet exists with no astro fields at all, and that is valid.
    set(&mut log, EntityId::PLANET, vec![("realism", f(0.5))]);
    for bad in [
        ("year_days", f(0.0)),
        ("day_hours", f(-1.0)),
        ("axial_tilt_deg", f(91.0)),
        ("axial_tilt_deg", f(-1.0)),
        ("eccentricity", f(0.99)),
        ("radius_km", f(0.0)),
        ("gravity_ms2", f(0.0)),
        ("order", i(0)),
        ("retrograde", f(1.0)),
    ] {
        let mut tx = log.transact("bad");
        tx.set(EntityId::PLANET, bad.0, bad.1.clone());
        assert!(tx.commit().is_err(), "planet {bad:?} was accepted");
    }
    set(
        &mut log,
        EntityId::PLANET,
        vec![
            ("year_days", f(390.0)),
            ("day_hours", f(30.0)),
            ("axial_tilt_deg", f(23.5)),
            ("retrograde", Value::Bool(false)),
            ("radius_km", f(7_200.0)),
            ("gravity_ms2", f(10.1)),
        ],
    );
}

#[test]
fn an_integer_is_accepted_where_a_number_is_wanted() {
    let mut log = new_log();
    assert!(
        try_add(
            &mut log,
            "astro.body",
            vec![
                ("name", t("X")),
                ("order", i(1)),
                ("class", t("rocky")),
                ("distance_au", i(5)),
            ],
        )
        .is_ok(),
        "distance_au = 5 means 5.0"
    );
}

#[test]
fn numeric_ceilings_reject_absurd_values() {
    let mut log = new_log();
    let body = |extra: (&'static str, Value)| {
        vec![
            ("name", t("X")),
            ("order", i(1)),
            ("class", t("rocky")),
            ("distance_au", f(1.0)),
            extra,
        ]
    };
    for bad in [
        ("distance_au", f(1e9 + 1.0)),
        ("period_years", f(1e9 + 1.0)),
        ("distance_km", f(1e12 + 1.0)),
        ("period_days", f(1e9 + 1.0)),
        ("radius_km", f(1e12 + 1.0)),
        ("density_kgm3", f(1e6 + 1.0)),
    ] {
        assert!(
            try_add(&mut log, "astro.body", body(bad.clone())).is_err(),
            "{bad:?} over its ceiling was accepted"
        );
    }
    // A belt's inner_au/outer_au share distance_au's ceiling.
    assert!(
        try_add(
            &mut log,
            "astro.body",
            vec![
                ("name", t("Belt")),
                ("order", i(2)),
                ("class", t("belt")),
                ("inner_au", f(1e9 + 1.0)),
                ("outer_au", f(1e9 + 2.0)),
            ],
        )
        .is_err(),
        "a belt extent over the ceiling was accepted"
    );
    // A star's mass_solar has its own ceiling.
    assert!(
        try_add(
            &mut log,
            "astro.star",
            vec![
                ("name", t("S")),
                ("role", t("primary")),
                ("mass_solar", f(1e6 + 1.0)),
            ],
        )
        .is_err(),
        "a star over the mass ceiling was accepted"
    );
    // The planet's year_days and day_hours.
    let mut tx = log.transact("bad");
    tx.set(EntityId::PLANET, "year_days", f(1e6 + 1.0));
    assert!(
        tx.commit().is_err(),
        "year_days over its ceiling was accepted"
    );
    let mut tx = log.transact("bad");
    tx.set(EntityId::PLANET, "day_hours", f(1e6 + 1.0));
    assert!(
        tx.commit().is_err(),
        "day_hours over its ceiling was accepted"
    );
    // A month's days, and a calendar's leap_every_n.
    let cal = add(
        &mut log,
        "astro.calendar",
        vec![("epoch_name", t("AE")), ("weekdays", list(&["A"]))],
    );
    assert!(
        try_add(
            &mut log,
            "astro.month",
            vec![
                ("index", i(1)),
                ("name", t("Huge")),
                ("days", i(100_001)),
                ("calendar", Value::Entity(cal)),
            ],
        )
        .is_err(),
        "a month over the days ceiling was accepted"
    );
    assert!(
        try_add(
            &mut log,
            "astro.calendar",
            vec![
                ("epoch_name", t("AE2")),
                ("weekdays", list(&["A"])),
                ("leap_rule", t("every_n")),
                ("leap_every_n", i(10_001)),
            ],
        )
        .is_err(),
        "leap_every_n over its ceiling was accepted"
    );
    // Every ceiling still admits a value right at it.
    assert!(
        try_add(
            &mut log,
            "astro.month",
            vec![
                ("index", i(2)),
                ("name", t("MaxLong")),
                ("days", i(100_000)),
                ("calendar", Value::Entity(cal)),
            ],
        )
        .is_ok(),
        "a month at exactly the ceiling was rejected"
    );
}

#[test]
fn the_two_former_calendar_panics_are_now_validator_rejections() {
    let mut log = new_log();
    let cal = add(
        &mut log,
        "astro.calendar",
        vec![("epoch_name", t("AE")), ("weekdays", list(&["A"]))],
    );
    // Two months of i64::MAX/2 days used to overflow `CalendarModel::month_days`'s sum
    // (`calendar.rs:67`); the ceiling now rejects either one long before the two could
    // ever be summed.
    assert!(
        try_add(
            &mut log,
            "astro.month",
            vec![
                ("index", i(1)),
                ("name", t("Huge1")),
                ("days", i(i64::MAX / 2)),
                ("calendar", Value::Entity(cal)),
            ],
        )
        .is_err(),
        "an overflow-sized month was accepted"
    );
    assert!(
        try_add(
            &mut log,
            "astro.month",
            vec![
                ("index", i(2)),
                ("name", t("Huge2")),
                ("days", i(i64::MAX / 2)),
                ("calendar", Value::Entity(cal)),
            ],
        )
        .is_err(),
        "a second overflow-sized month was accepted"
    );
    // `leap_every_n = i64::MAX` used to overflow `n * base + 1` in `date_of`
    // (`calendar.rs:127`); the ceiling now rejects the calendar before that arithmetic
    // ever runs.
    assert!(
        try_add(
            &mut log,
            "astro.calendar",
            vec![
                ("epoch_name", t("AE2")),
                ("weekdays", list(&["A"])),
                ("leap_rule", t("every_n")),
                ("leap_every_n", i(i64::MAX)),
            ],
        )
        .is_err(),
        "leap_every_n = i64::MAX was accepted"
    );
}
