mod common;

use common::{
    add, add_aethorian_calendar, add_gregorian_calendar, aethoria, earth_system, f, i, named,
    new_log, set, t,
};
use wb_astro::{AU_M, AstroError, BodyClass, SkyModel};
use wb_editlog::{EntityId, Value};

fn close(got: f64, want: f64, rel: f64) {
    let err = (got - want).abs() / want.abs();
    assert!(
        err <= rel,
        "got {got}, want {want} (relative error {err} > {rel})"
    );
}

#[test]
fn a_system_with_no_primary_star_is_an_error() {
    let mut log = new_log();
    add_gregorian_calendar(&mut log);
    assert_eq!(
        SkyModel::build(log.state()).err(),
        Some(AstroError::NoPrimaryStar)
    );
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("A")),
            ("role", t("companion")),
            ("mass_solar", f(1.0)),
        ],
    );
    assert_eq!(
        SkyModel::build(log.state()).err(),
        Some(AstroError::NoPrimaryStar),
        "a companion alone has nothing to be a companion to"
    );
}

#[test]
fn two_primary_stars_are_an_error() {
    let mut log = earth_system();
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Other")),
            ("role", t("primary")),
            ("mass_solar", f(1.0)),
        ],
    );
    assert_eq!(
        SkyModel::build(log.state()).err(),
        Some(AstroError::MultiplePrimaryStars)
    );
}

#[test]
fn a_missing_parent_is_an_error() {
    let mut log = earth_system();
    let ghost = EntityId {
        op: wb_editlog::OpId {
            lamport: 9_999,
            actor: wb_editlog::ActorId([7; 16]),
        },
        n: 0,
    };
    let orphan = add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Orphan")),
            ("order", i(1)),
            ("class", t("moon")),
            ("parent", Value::Entity(ghost)),
            ("period_days", f(5.0)),
        ],
    );
    assert_eq!(
        SkyModel::build(log.state()).err(),
        Some(AstroError::MissingParent(orphan))
    );
}

#[test]
fn a_parent_cycle_is_an_error() {
    let mut log = earth_system();
    let a = add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("A")),
            ("order", i(1)),
            ("class", t("rocky")),
            ("distance_au", f(2.0)),
        ],
    );
    let b = add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("B")),
            ("order", i(2)),
            ("class", t("rocky")),
            ("distance_au", f(3.0)),
            ("parent", Value::Entity(a)),
        ],
    );
    set(&mut log, a, vec![("parent", Value::Entity(b))]);
    let err = SkyModel::build(log.state()).err();
    assert!(
        err == Some(AstroError::ParentCycle(a)) || err == Some(AstroError::ParentCycle(b)),
        "expected a cycle error, got {err:?}"
    );
}

#[test]
fn a_system_with_no_calendar_or_no_months_is_an_error() {
    let mut log = new_log();
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("S")),
            ("role", t("primary")),
            ("mass_solar", f(1.0)),
        ],
    );
    assert_eq!(
        SkyModel::build(log.state()).err(),
        Some(AstroError::NoCalendar)
    );
    let cal = add(
        &mut log,
        "astro.calendar",
        vec![("epoch_name", t("AE")), ("weekdays", common::list(&["A"]))],
    );
    assert_eq!(
        SkyModel::build(log.state()).err(),
        Some(AstroError::NoMonths)
    );
    add(
        &mut log,
        "astro.month",
        vec![
            ("index", i(1)),
            ("name", t("M")),
            ("days", i(30)),
            ("calendar", Value::Entity(cal)),
        ],
    );
    assert!(SkyModel::build(log.state()).is_ok());
}

#[test]
fn the_home_world_sits_where_its_year_puts_it() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let home = sky.home();
    assert!(home.is_home);
    assert_eq!(home.id, EntityId::PLANET);
    assert_eq!(home.class, BodyClass::Home);
    // A 365.2422-day, 24-hour year around a solar-mass star is 1 AU.
    close(home.a_m, AU_M, 0.001);
    close(home.mass_kg, 5.972e24, 0.01);
    close(sky.year_days(), 365.2422, 1e-12);
    close(sky.day_hours(), 24.0, 1e-12);
    close(sky.axial_tilt_rad(), 23.44_f64.to_radians(), 1e-12);
    assert!(!sky.is_retrograde());
}

#[test]
fn a_distance_implies_a_period_and_a_period_implies_a_distance() {
    let mut log = earth_system();
    // Jupiter by distance; its period should come out near 11.86 Earth years.
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Jove")),
            ("order", i(5)),
            ("class", t("gas_giant")),
            ("distance_au", f(5.2044)),
        ],
    );
    // Saturn by period, in local years.
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Cronos")),
            ("order", i(6)),
            ("class", t("gas_giant")),
            ("period_years", f(29.457)),
        ],
    );
    let sky = SkyModel::build(log.state()).unwrap();
    let jove = sky.body(named(&log, "Jove")).unwrap();
    let year_s = 365.2422 * 24.0 * 3_600.0;
    close(jove.period_s / year_s, 11.862, 0.01);
    assert!(jove.stated_a_m.is_some());
    assert_eq!(jove.stated_period_s, None);

    let cronos = sky.body(named(&log, "Cronos")).unwrap();
    close(cronos.a_m / AU_M, 9.5826, 0.01);
    assert_eq!(cronos.stated_a_m, None);
}

#[test]
fn a_moons_cycle_is_in_local_days() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let luna = sky.moons_of_home().next().expect("Luna");
    assert_eq!(luna.name, "Luna");
    close(luna.a_m, 3.844e8, 1e-6);
    // 384 400 km around Earth is 27.32 days of 24 hours.
    close(luna.period_s / 86_400.0, 27.32, 0.01);
    close(luna.mass_kg, 7.35e22, 0.02);
}

#[test]
fn class_defaults_fill_in_what_the_user_did_not_say() {
    let mut log = earth_system();
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Vague")),
            ("order", i(9)),
            ("class", t("gas_giant")),
            ("distance_au", f(12.0)),
        ],
    );
    let sky = SkyModel::build(log.state()).unwrap();
    let v = sky.body(named(&log, "Vague")).unwrap();
    close(v.radius_m, 60_000_000.0, 1e-12);
    close(v.density_kgm3, 1_300.0, 1e-12);
    close(v.albedo, 0.5, 1e-12);
    assert_eq!(v.luminosity_w, None);
}

#[test]
fn a_star_without_a_stated_luminosity_gets_one_from_its_mass() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    let secundus = sky
        .stars()
        .find(|s| s.name == "Sol Secundus")
        .expect("the companion");
    let l = secundus.luminosity_w.expect("stars radiate");
    // L = M^3.5 for M = 0.33 is about 0.0206 solar luminosities.
    close(l / wb_astro::SOLAR_LUMINOSITY_W, 0.0206, 0.05);
    assert!(!secundus.is_primary_star);
    assert!(sky.primary_star().is_primary_star);
    assert_eq!(sky.primary_star().name, "Sol Prime");
    assert_eq!(sky.stars().count(), 2);
}

#[test]
fn bodies_are_listed_in_entity_order_and_include_the_stars_and_the_home_world() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let ids: Vec<EntityId> = sky.bodies().map(|b| b.id).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "bodies() must iterate in EntityId order");
    // 13 ordered bodies (12 entities + the home world) + 3 moons + 2 stars.
    assert_eq!(ids.len(), 18);
    assert!(ids.contains(&EntityId::PLANET));
    assert_eq!(sky.moons_of_home().count(), 3);
}

#[test]
fn a_belt_keeps_its_extent_and_orbits_at_its_middle() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let veil = sky.body(named(&log, "The Veil")).unwrap();
    assert_eq!(veil.class, BodyClass::Belt);
    let (inner, outer) = veil.extent_m.expect("a belt has an extent");
    close(inner / AU_M, 2.6, 1e-9);
    close(outer / AU_M, 3.4, 1e-9);
    close(veil.a_m / AU_M, 3.0, 1e-9);
}

#[test]
fn positions_are_relative_to_the_home_world_and_move_through_the_year() {
    let log = earth_system();
    let sky = SkyModel::build(log.state()).unwrap();
    let sun = sky.primary_star().id;
    // The Sun sits one AU away, on the opposite side from the home world's own position.
    let d = sky.position(sun, 0.0).expect("the Sun has a position");
    close(d.length(), AU_M, 0.02);
    let helio = sky.heliocentric(EntityId::PLANET, 0.0).unwrap();
    close(helio.length(), AU_M, 0.001);
    assert_eq!(sky.heliocentric(sun, 0.0).map(|v| v.length()), Some(0.0));

    // Half a year later the home world is on the other side of its star.
    let later = sky.heliocentric(EntityId::PLANET, 365.2422 / 2.0).unwrap();
    assert!(
        helio.x * later.x < 0.0 || helio.y * later.y < 0.0,
        "half a year should put the world across the system: {helio:?} then {later:?}"
    );

    // The Moon stays near the home world, not near the Sun.
    let luna = sky.moons_of_home().next().unwrap().id;
    close(sky.position(luna, 0.0).unwrap().length(), 3.844e8, 1e-6);
    assert_eq!(
        sky.position(EntityId::PLANET, 10.0),
        Some(wb_grid::Vec3::new(0.0, 0.0, 0.0))
    );
}

#[test]
fn an_unknown_id_has_no_position() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let ghost = EntityId {
        op: wb_editlog::OpId {
            lamport: 1_234,
            actor: wb_editlog::ActorId([3; 16]),
        },
        n: 2,
    };
    assert_eq!(sky.position(ghost, 0.0), None);
    assert_eq!(sky.heliocentric(ghost, 0.0), None);
    assert_eq!(sky.body(ghost), None);
}

#[test]
fn deleted_entities_are_not_in_the_system() {
    let mut log = aethoria();
    let veil = named(&log, "The Veil");
    let mut tx = log.transact("delete");
    tx.delete(veil);
    tx.commit().unwrap();
    let sky = SkyModel::build(log.state()).unwrap();
    assert_eq!(sky.body(veil), None);
    assert_eq!(sky.bodies().count(), 17);
}

#[test]
fn the_calendar_is_read_with_its_months_in_index_order() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let cal = sky.calendar();
    assert_eq!(cal.epoch_name, "AE");
    assert_eq!(cal.weekdays.len(), 7);
    assert_eq!(cal.weekdays[0], "Solday");
    assert_eq!(cal.months.len(), 13);
    assert_eq!(cal.months[0].name, "Mehr");
    assert_eq!(cal.months[12].name, "Vasanta");
    assert_eq!(cal.month_days(), 390);
    assert_eq!(cal.leap_every_n, None);

    let mut earth = common::new_log();
    add(
        &mut earth,
        "astro.star",
        vec![
            ("name", t("S")),
            ("role", t("primary")),
            ("mass_solar", f(1.0)),
        ],
    );
    add_gregorian_calendar(&mut earth);
    let sky = SkyModel::build(earth.state()).unwrap();
    assert_eq!(sky.calendar().month_days(), 365);
    assert_eq!(sky.calendar().leap_every_n, Some(4));
    assert!(sky.calendar().leap_month.is_some());
}

#[test]
fn an_unstated_planet_gets_earth_like_defaults() {
    let mut log = new_log();
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("S")),
            ("role", t("primary")),
            ("mass_solar", f(1.0)),
        ],
    );
    add_aethorian_calendar(&mut log);
    let sky = SkyModel::build(log.state()).unwrap();
    close(sky.year_days(), 365.2422, 1e-12);
    close(sky.day_hours(), 24.0, 1e-12);
    close(sky.home().radius_m, 6_371_000.0, 1e-12);
    close(sky.home().a_m, AU_M, 0.001);
}

#[test]
fn an_absurd_distance_does_not_become_infinity() {
    // The validator ceiling (kinds.rs) now rejects anything past 1e9 AU outright, which
    // closes off the overflow this test used to probe for with 1e300. The remaining
    // question is the boundary itself: does the largest value a user can still enter
    // stay finite all the way through? That is what this now checks.
    let mut log = earth_system();
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Absurd")),
            ("order", i(9)),
            ("class", t("rocky")),
            ("distance_au", f(1e9)),
        ],
    );
    let sky = SkyModel::build(log.state()).unwrap();
    let b = sky.body(named(&log, "Absurd")).unwrap();
    assert!(b.a_m.is_finite(), "a_m = {}", b.a_m);
    assert!(b.period_s.is_finite(), "period_s = {}", b.period_s);
    let p = sky.position(b.id, 3.0).expect("a position");
    assert!(
        p.x.is_finite() && p.y.is_finite() && p.z.is_finite(),
        "{p:?}"
    );
}

#[test]
fn absurd_masses_and_radii_do_not_become_infinity() {
    // Same story as the distance test above: the validator ceilings (mass_solar ≤ 1e6,
    // radius_km ≤ 1e12) now reject 1e300/1e100 outright, so this checks the largest
    // values still admitted stay finite through the downstream arithmetic.
    let mut log = new_log();
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Monstrous")),
            ("role", t("primary")),
            ("mass_solar", f(1e6)),
        ],
    );
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Bloated")),
            ("order", i(1)),
            ("class", t("rocky")),
            ("distance_au", f(1.0)),
            ("radius_km", f(1e12)),
        ],
    );
    add_aethorian_calendar(&mut log);
    let sky = SkyModel::build(log.state()).unwrap();
    for b in sky.bodies() {
        assert!(b.mass_kg.is_finite(), "{} mass_kg = {}", b.name, b.mass_kg);
        assert!(
            b.radius_m.is_finite(),
            "{} radius_m = {}",
            b.name,
            b.radius_m
        );
        assert!(b.a_m.is_finite(), "{} a_m = {}", b.name, b.a_m);
        assert!(
            b.period_s.is_finite(),
            "{} period_s = {}",
            b.name,
            b.period_s
        );
        assert!(
            b.luminosity_w.map(f64::is_finite).unwrap_or(true),
            "{} luminosity_w = {:?}",
            b.name,
            b.luminosity_w
        );
        let p = sky.position(b.id, 7.0).expect("a position");
        assert!(
            p.x.is_finite() && p.y.is_finite() && p.z.is_finite(),
            "{} {p:?}",
            b.name
        );
    }
}
