mod common;

use common::{add, aethoria, earth_system, f, i, named, new_log, t};
use proptest::prelude::*;
use wb_astro::{Date, SkyModel};
use wb_editlog::Value;

#[test]
fn the_epoch_is_year_one_day_one() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    let d = sky.date_of(0);
    assert_eq!(
        d,
        Date {
            year: 1,
            month_index: 1,
            day: 1,
            weekday_index: 0
        }
    );
    assert_eq!(sky.day_index_of(d), Some(0));
}

#[test]
fn a_thirteen_month_year_of_thirty_day_months_counts_through() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    assert_eq!(sky.date_of(29).day, 30);
    assert_eq!(sky.date_of(29).month_index, 1);
    assert_eq!(sky.date_of(30).day, 1);
    assert_eq!(sky.date_of(30).month_index, 2);
    assert_eq!(sky.date_of(389).month_index, 13);
    assert_eq!(sky.date_of(389).day, 30);
    assert_eq!(sky.date_of(389).year, 1);
    assert_eq!(sky.date_of(390).year, 2);
    assert_eq!(sky.date_of(390).month_index, 1);
    assert_eq!(sky.date_of(390).day, 1);
}

#[test]
fn the_seven_day_week_cycles_independently_of_the_months() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    for d in 0..40 {
        assert_eq!(sky.date_of(d).weekday_index, (d % 7) as usize);
    }
    // 390 is not a multiple of 7, so a year does not start on the same weekday.
    assert_eq!(sky.date_of(390).weekday_index, 390 % 7);
    assert_eq!(
        sky.calendar().weekdays[sky.date_of(0).weekday_index],
        "Solday"
    );
}

#[test]
fn days_before_the_epoch_run_backwards() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    let d = sky.date_of(-1);
    assert_eq!(d.year, 0);
    assert_eq!(d.month_index, 13);
    assert_eq!(d.day, 30);
    assert_eq!(sky.day_index_of(d), Some(-1));
    assert_eq!(sky.date_of(-390).year, 0);
    assert_eq!(sky.date_of(-390).month_index, 1);
    assert_eq!(sky.date_of(-391).year, -1);
}

#[test]
fn a_leap_rule_lengthens_the_right_month_of_the_right_year() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let cal = sky.calendar();
    assert_eq!(cal.month_days(), 365);
    assert!(!cal.is_leap(1));
    assert!(!cal.is_leap(3));
    assert!(cal.is_leap(4), "every 4th year from the epoch");
    assert!(cal.is_leap(8));
    assert!(cal.is_leap(0));
    assert_eq!(cal.year_length(1), 365);
    assert_eq!(cal.year_length(4), 366);
    // February gains the day, so the 60th day of a leap year is 29 February.
    let start_of_four = cal.start_of_year(4).unwrap();
    assert_eq!(start_of_four, 3 * 365);
    let feb29 = sky.date_of(start_of_four + 59);
    assert_eq!((feb29.month_index, feb29.day), (2, 29));
    let mar1 = sky.date_of(start_of_four + 60);
    assert_eq!((mar1.month_index, mar1.day), (3, 1));
    // The same offset in a common year is already 1 March.
    let common = sky.date_of(cal.start_of_year(5).unwrap() + 59);
    assert_eq!((common.month_index, common.day), (3, 1));
}

#[test]
fn a_leap_day_with_no_named_month_falls_at_the_end_of_the_year() {
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
    let cal = add(
        &mut log,
        "astro.calendar",
        vec![
            ("epoch_name", t("AE")),
            ("weekdays", common::list(&["A", "B"])),
            ("leap_rule", t("every_n")),
            ("leap_every_n", i(3)),
        ],
    );
    for n in 1..=2 {
        add(
            &mut log,
            "astro.month",
            vec![
                ("index", i(n)),
                ("name", t("M")),
                ("days", i(10)),
                ("calendar", Value::Entity(cal)),
            ],
        );
    }
    let sky = SkyModel::build(log.state()).unwrap();
    assert_eq!(sky.calendar().year_length(1), 20);
    assert_eq!(sky.calendar().year_length(3), 21);
    let last = sky.date_of(sky.calendar().start_of_year(3).unwrap() + 20);
    assert_eq!((last.month_index, last.day), (2, 11));
}

#[test]
fn dates_round_trip_over_a_hundred_thousand_consecutive_days() {
    for log in [aethoria(), earth_system()] {
        let sky = SkyModel::build(log.state()).unwrap();
        let mut previous: Option<Date> = None;
        for d in -50_000..50_000 {
            let date = sky.date_of(d);
            assert_eq!(
                sky.day_index_of(date),
                Some(d),
                "{date:?} did not round-trip"
            );
            assert!(date.day >= 1, "{date:?}");
            if let Some(p) = previous {
                // Monotonic: a later day index is never an earlier date.
                assert!(
                    (date.year, date.month_index, date.day) > (p.year, p.month_index, p.day),
                    "{p:?} then {date:?}"
                );
            }
            previous = Some(date);
        }
    }
}

#[test]
fn an_impossible_date_has_no_day_index() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    let base = Date {
        year: 3,
        month_index: 2,
        day: 7,
        weekday_index: 0,
    };
    assert!(sky.day_index_of(base).is_some());
    for bad in [
        Date {
            month_index: 14,
            ..base
        },
        Date {
            month_index: 0,
            ..base
        },
        Date { day: 31, ..base },
        Date { day: 0, ..base },
        Date { day: -5, ..base },
        Date {
            year: i64::MAX,
            ..base
        },
        Date {
            year: i64::MIN,
            ..base
        },
    ] {
        assert_eq!(sky.day_index_of(bad), None, "{bad:?} was accepted");
    }
    // The weekday is derived, so a wrong one is ignored rather than rejected.
    assert_eq!(
        sky.day_index_of(Date {
            weekday_index: 99,
            ..base
        }),
        sky.day_index_of(base)
    );
}

#[test]
fn ten_million_days_out_still_round_trips() {
    // Both calendars: the no-leap Aethorian one and the leap-every-4 Gregorian one,
    // because the leap rule is what makes the cycle decomposition non-trivial.
    for log in [aethoria(), earth_system()] {
        let sky = SkyModel::build(log.state()).unwrap();
        for d in [-10_000_000, -9_999_999, -1, 0, 1, 1_000_001, 10_000_000] {
            assert_eq!(sky.day_index_of(sky.date_of(d)), Some(d), "day {d}");
        }
    }
}

#[test]
fn a_season_is_found_by_its_named_range() {
    let mut log = aethoria();
    let cal = log
        .state()
        .live()
        .find(|e| e.kind() == Some("astro.calendar"))
        .map(|e| e.id)
        .expect("the Aethorian calendar");
    let mehr = named(&log, "Mehr");
    let shahrivar = named(&log, "Shahrivar");
    let vasanta = named(&log, "Vasanta");
    add(
        &mut log,
        "astro.season",
        vec![
            ("name", t("Primary Summer")),
            ("start_month", Value::Entity(mehr)),
            ("start_day", i(1)),
            ("end_month", Value::Entity(shahrivar)),
            ("end_day", i(30)),
            ("calendar", Value::Entity(cal)),
        ],
    );
    // A season that wraps the year end: Vasanta (13) into Mehr (1).
    add(
        &mut log,
        "astro.season",
        vec![
            ("name", t("Turning")),
            ("start_month", Value::Entity(vasanta)),
            ("start_day", i(20)),
            ("end_month", Value::Entity(mehr)),
            ("end_day", i(5)),
            ("calendar", Value::Entity(cal)),
        ],
    );
    let sky = SkyModel::build(log.state()).unwrap();
    assert_eq!(sky.season_of(0.0), Some("Primary Summer"));
    assert_eq!(sky.season_of(89.0), Some("Primary Summer"));
    assert_eq!(sky.season_of(90.0), None, "nothing covers Tejas");
    // Day 379 is Vasanta 20 (12 months × 30 = 360, + 19).
    assert_eq!(sky.season_of(379.0), Some("Turning"));
    assert_eq!(sky.season_of(389.0), Some("Turning"));
    assert_eq!(sky.season_of(378.0), None);
    // A fractional day is the day it falls in.
    assert_eq!(sky.season_of(379.7), Some("Turning"));
    // Out-of-range days wrap into the year rather than vanishing.
    assert_eq!(sky.season_of(390.0), sky.season_of(0.0));
    assert_eq!(sky.season_of(-1.0), sky.season_of(389.0));
}

#[test]
fn a_world_with_no_seasons_has_none() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    assert_eq!(sky.calendar().seasons.len(), 0);
    for d in [0.0, 100.0, 389.0] {
        assert_eq!(sky.season_of(d), None);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn every_day_index_round_trips(d in -10_000_000i64..10_000_000) {
        let log = aethoria();
        let sky = SkyModel::build(log.state()).unwrap();
        prop_assert_eq!(sky.day_index_of(sky.date_of(d)), Some(d));
    }

    #[test]
    fn every_day_index_round_trips_with_a_leap_rule(d in -10_000_000i64..10_000_000) {
        let log = earth_system();
        let sky = SkyModel::build(log.state()).unwrap();
        prop_assert_eq!(sky.day_index_of(sky.date_of(d)), Some(d));
    }

    #[test]
    fn a_date_is_always_inside_its_month(d in -1_000_000i64..1_000_000) {
        let log = earth_system();
        let sky = SkyModel::build(log.state()).unwrap();
        let date = sky.date_of(d);
        let cal = sky.calendar();
        let position = cal
            .months
            .iter()
            .position(|m| m.index == date.month_index)
            .expect("date_of names a real month");
        prop_assert!(date.day >= 1);
        prop_assert!(date.day <= cal.days_in(position, cal.is_leap(date.year)));
        prop_assert!(date.weekday_index < cal.weekdays.len());
    }
}
