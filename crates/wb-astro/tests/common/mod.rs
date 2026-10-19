#![allow(dead_code)]
use wb_astro::register_kinds;
use wb_editlog::{ActorId, AuthorId, EditError, EditLog, EntityId, FixedClock, Value};

pub fn new_log() -> EditLog {
    let mut log = EditLog::new(
        ActorId([1; 16]),
        AuthorId([9; 16]),
        Box::new(FixedClock(1_758_000_000_000)),
    );
    register_kinds(&mut log);
    log
}

/// Creates an entity, panicking if it is rejected — for fixtures that must be valid.
pub fn add(log: &mut EditLog, kind: &str, fields: Vec<(&str, Value)>) -> EntityId {
    try_add(log, kind, fields).expect("fixture entity should be valid")
}

/// Creates an entity, returning the validator's verdict — for validator tests.
pub fn try_add(
    log: &mut EditLog,
    kind: &str,
    fields: Vec<(&str, Value)>,
) -> Result<EntityId, EditError> {
    let mut tx = log.transact("add");
    let e = tx.create(kind, fields);
    tx.commit().map(|_| e)
}

/// Sets fields on an existing entity (the planet, usually).
pub fn set(log: &mut EditLog, entity: EntityId, fields: Vec<(&str, Value)>) {
    let mut tx = log.transact("set");
    for (f, v) in fields {
        tx.set(entity, f, v);
    }
    tx.commit().expect("fixture writes should be valid");
}

pub fn f(x: f64) -> Value {
    Value::Float(x)
}

pub fn i(n: i64) -> Value {
    Value::Int(n)
}

pub fn t(s: &str) -> Value {
    Value::from(s)
}

pub fn list(items: &[&str]) -> Value {
    Value::List(items.iter().map(|s| Value::from(*s)).collect())
}

/// The real Sun–Earth–Moon system, in this engine's friendly fields: the yardstick every
/// validation test measures against.
pub fn earth_system() -> EditLog {
    let mut log = new_log();
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Sol")),
            ("role", t("primary")),
            ("mass_solar", f(1.0)),
            ("luminosity_solar", f(1.0)),
        ],
    );
    set(
        &mut log,
        EntityId::PLANET,
        vec![
            ("order", i(3)),
            ("year_days", f(365.2422)),
            ("day_hours", f(24.0)),
            ("axial_tilt_deg", f(23.44)),
            ("radius_km", f(6_371.0)),
            ("gravity_ms2", f(9.80665)),
        ],
    );
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Luna")),
            ("order", i(1)),
            ("class", t("moon")),
            ("parent", Value::Entity(EntityId::PLANET)),
            ("distance_km", f(384_400.0)),
            ("radius_km", f(1_737.4)),
            ("density_kgm3", f(3_344.0)),
        ],
    );
    add_gregorian_calendar(&mut log);
    log
}

/// Twelve months of 30/31 days summing to 365, a seven-day week, leap every 4 years.
pub fn add_gregorian_calendar(log: &mut EditLog) -> EntityId {
    let cal = add(
        log,
        "astro.calendar",
        vec![
            ("epoch_name", t("CE")),
            (
                "weekdays",
                list(&["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]),
            ),
            ("leap_rule", t("every_n")),
            ("leap_every_n", i(4)),
        ],
    );
    let lengths = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut february = None;
    for (n, days) in lengths.iter().enumerate() {
        let m = add(
            log,
            "astro.month",
            vec![
                ("index", i(n as i64 + 1)),
                ("name", t(MONTH_NAMES[n])),
                ("days", i(*days)),
                ("calendar", Value::Entity(cal)),
            ],
        );
        if n == 1 {
            february = Some(m);
        }
    }
    set(
        log,
        cal,
        vec![(
            "leap_month",
            Value::Entity(february.expect("February exists")),
        )],
    );
    cal
}

pub const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Aethoria: thirteen bodies, two suns, three moons, a 30-hour day and a 390-day year.
/// Distances are this engine's reading of the lore, not canon: the lore fixes the order,
/// the two suns, the three moons' cycles and the calendar, and nothing else.
pub fn aethoria() -> EditLog {
    let mut log = new_log();
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Sol Prime")),
            ("role", t("primary")),
            // A 390-day year of 30-hour days is 1.33 Earth years, which puts Aethoria at
            // 1.29 AU. A sun-like star would leave it at 231 K — frozen — so Sol Prime is
            // the brighter end of "similar in size to Earth's sun": 1.2 M☉, 1.9 L☉
            // (which is also what M^3.5 gives, so the two fields agree).
            ("mass_solar", f(1.2)),
            ("luminosity_solar", f(1.9)),
        ],
    );
    // "Smaller sun (1/3 size), orbiting in outer system", between Goliath and Stormheart.
    // `period_years = 1` is the lore's "it returns every year" entered as data, which is
    // what makes the clash with a 6 AU orbit explicit (spec §6).
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Sol Secundus")),
            ("role", t("companion")),
            ("mass_solar", f(0.33)),
            ("order", i(8)),
            ("distance_au", f(6.0)),
            ("period_years", f(1.0)),
            ("eccentricity", f(0.1)),
        ],
    );
    set(
        &mut log,
        EntityId::PLANET,
        vec![
            ("name", t("Aethoria")),
            ("order", i(5)),
            ("year_days", f(390.0)),
            ("day_hours", f(30.0)),
            ("axial_tilt_deg", f(24.0)),
            ("radius_km", f(7_200.0)),
            ("gravity_ms2", f(10.2)),
        ],
    );
    // order, name, class, distance_au. The home world's 390-day year puts it at 1.29 AU
    // about a 1.2 M☉ star, so the inner four sit below that and Rubros above it.
    // Stormheart is at 5.5 × 2^(2/3), an exact 2:1 with Goliath — the lore's "Sol
    // Secundus orbital resonance", and what the resonance checker is asserted on.
    let planets: [(i64, &str, &str, f64); 12] = [
        (1, "Pyrox", "rocky", 0.45),
        (2, "Luminar", "rocky", 0.75),
        (3, "Verdant", "rocky", 0.95),
        (4, "Azuros", "water", 1.05),
        (6, "Rubros", "rocky", 1.7),
        (7, "The Veil", "belt", 3.0),
        (8, "Goliath", "gas_giant", 5.5),
        (9, "Stormheart", "gas_giant", 8.7307),
        (10, "Frostweaver", "ice_giant", 15.0),
        (11, "Shadowkeep", "gas_giant", 22.0),
        (12, "Rimefrost", "ice_giant", 31.0),
        // "Highly elliptical": its perihelion reaches inside Rimefrost's orbit, with no
        // resonance to excuse it — Aethoria's one honest orbit_crossing.
        (13, "Voidwalker", "rocky", 55.0),
    ];
    for (order, name, class, au) in planets {
        let mut fields = vec![("name", t(name)), ("order", i(order)), ("class", t(class))];
        if class == "belt" {
            fields.push(("inner_au", f(au - 0.4)));
            fields.push(("outer_au", f(au + 0.4)));
        } else {
            fields.push(("distance_au", f(au)));
        }
        if name == "Stormheart" {
            fields.push(("rings", Value::Bool(true)));
        }
        if name == "Frostweaver" {
            fields.push(("inclination_deg", f(12.0)));
        }
        if name == "Voidwalker" {
            fields.push(("eccentricity", f(0.45)));
        }
        add(&mut log, "astro.body", fields);
    }
    // The three moons, by their lore cycles in local days.
    for (order, name, cycle) in [
        (1, "Lumara's Tear", 10.0),
        (2, "Malakai's Eye", 15.0),
        (3, "Zyraxis' Crown", 30.0),
    ] {
        add(
            &mut log,
            "astro.body",
            vec![
                ("name", t(name)),
                ("order", i(order)),
                ("class", t("moon")),
                ("parent", Value::Entity(EntityId::PLANET)),
                ("period_days", f(cycle)),
            ],
        );
    }
    add_aethorian_calendar(&mut log);
    log
}

/// Thirteen months of thirty days, the seven-day Septem, no leap rule.
pub fn add_aethorian_calendar(log: &mut EditLog) -> EntityId {
    let cal = add(
        log,
        "astro.calendar",
        vec![
            ("epoch_name", t("AE")),
            (
                "weekdays",
                list(&[
                    "Solday", "Lumaday", "Maladay", "Zyraday", "Narday", "Pyrday", "Secunday",
                ]),
            ),
        ],
    );
    for (n, name) in AETHORIAN_MONTHS.iter().enumerate() {
        add(
            log,
            "astro.month",
            vec![
                ("index", i(n as i64 + 1)),
                ("name", t(name)),
                ("days", i(30)),
                ("calendar", Value::Entity(cal)),
            ],
        );
    }
    cal
}

pub const AETHORIAN_MONTHS: [&str; 13] = [
    "Mehr",
    "Solspire",
    "Shahrivar",
    "Tejas",
    "Azar",
    "Geminsol",
    "Yugma",
    "Sandhya",
    "Mot",
    "Rajani",
    "Hemanta",
    "Himani",
    "Vasanta",
];

/// The id of the body with this name, for assertions.
pub fn named(log: &EditLog, name: &str) -> EntityId {
    log.state()
        .live()
        .find(|v| match v.get("name") {
            Some(Value::Text(n)) => n == name,
            _ => false,
        })
        .map(|v| v.id)
        .unwrap_or_else(|| panic!("no entity named {name}"))
}

use std::collections::BTreeSet;
use wb_astro::{register_catalog, with_astro_checks};
use wb_constraints::{
    CheckContext, CheckerRegistry, Finding, KindCatalog, Report, UnknownTerrain, evaluate,
};
use wb_world::Planet;

/// Evaluates a world with the astro kinds and checkers registered.
pub fn astro_report(log: &EditLog) -> Report {
    let mut catalog = KindCatalog::with_starter_kinds();
    register_catalog(&mut catalog);
    let mut registry = CheckerRegistry::new();
    with_astro_checks(&mut registry);
    let ctx = CheckContext::with_catalog(log.state(), Planet::default(), &UnknownTerrain, &catalog);
    evaluate(&ctx, &registry)
}

/// Every issue, support and consequence code in a report.
pub fn codes(r: &Report) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for v in &r.verdicts {
        for f in v
            .issues
            .iter()
            .chain(&v.intentional)
            .chain(&v.supports)
            .chain(&v.consequences)
        {
            out.insert(f.code.0.clone());
        }
    }
    out
}

/// Every finding with this code.
pub fn findings<'a>(r: &'a Report, code: &str) -> Vec<&'a Finding> {
    r.verdicts
        .iter()
        .flat_map(|v| {
            v.issues
                .iter()
                .chain(&v.intentional)
                .chain(&v.supports)
                .chain(&v.consequences)
        })
        .filter(|f| f.code.0 == code)
        .collect()
}
