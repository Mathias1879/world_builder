mod common;

use common::{
    add, add_aethorian_calendar, aethoria, astro_report, codes, earth_system, f, findings, i,
    named, new_log, set, t,
};
use wb_astro::{
    critical_distance_m, critical_fraction_outer, hill_radius_m, roche_limit_m, simple_ratio,
};
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
fn the_published_criteria_give_the_published_numbers() {
    // Holman & Wiegert 1999: an equal-mass circular binary allows S-type orbits out to
    // about 0.27 of the separation.
    close(critical_distance_m(1.0, 0.5, 0.0), 0.274, 0.02);
    // A light, distant companion is far more forgiving than a heavy, eccentric one.
    assert!(critical_distance_m(1.0, 0.1, 0.0) > critical_distance_m(1.0, 0.5, 0.4));

    // Earth's Hill radius is about 1.5 million km.
    close(hill_radius_m(1.496e11, 5.972e24, 1.98841e30), 1.496e9, 0.05);
    // The fluid (deformable) Roche limit about Earth is about 18,400 km. A moon above a
    // few hundred km is held together by gravity, not material strength, so this — not
    // the ~9,500 km rigid-body figure — is the threshold that matters for a real moon.
    close(roche_limit_m(6.371e6, 5_514.0, 3_344.0), 1.84e7, 0.05);
    assert_eq!(roche_limit_m(6.371e6, 5_514.0, 0.0), 0.0);
    assert_eq!(hill_radius_m(1.0, 0.0, 1.0), 0.0);
}

#[test]
fn simple_ratios_are_recognised_and_awkward_ones_are_not() {
    assert_eq!(simple_ratio(1.5), Some((3, 2)));
    assert_eq!(simple_ratio(2.0), Some((2, 1)));
    assert_eq!(simple_ratio(1.505), Some((3, 2)), "within 2%");
    assert_eq!(simple_ratio(1.0 / 1.5), Some((2, 3)));
    assert_eq!(simple_ratio(1.37), None);
    assert_eq!(simple_ratio(0.0), None);
    assert_eq!(simple_ratio(f64::NAN), None);
}

#[test]
fn a_tight_companion_destabilizes_the_planets() {
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
    let r = astro_report(&log);
    let found = findings(&r, "astro.companion_destabilizes");
    assert_eq!(found.len(), 1, "{:?}", codes(&r));
    let finding = found[0];
    assert_eq!(finding.kind, FindingKind::Issue);
    assert_eq!(finding.badness, 0.9);
    assert!(
        finding.related.contains(&EntityId::PLANET),
        "the home world is one of the bodies at risk"
    );
    assert!(matches!(
        finding.params.get("critical_au"),
        Some(Param::Float(_))
    ));
    // Both suggested fixes: move it out, or make it too light to be a star.
    assert_eq!(finding.suggestions.len(), 2);
    let codes: Vec<&str> = finding
        .suggestions
        .iter()
        .map(|s| s.code.as_str())
        .collect();
    assert!(codes.contains(&"astro.move_companion_out"));
    assert!(codes.contains(&"astro.demote_companion"));
    for s in &finding.suggestions {
        assert_eq!(s.writes.len(), 1);
        assert!(s.creates.is_empty());
    }
}

#[test]
fn applying_the_fix_clears_the_finding() {
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
    let r = astro_report(&log);
    let fix = findings(&r, "astro.companion_destabilizes")[0]
        .suggestions
        .iter()
        .find(|s| s.code.as_str() == "astro.move_companion_out")
        .expect("the move-out fix")
        .clone();
    wb_constraints::apply_suggestion(&mut log, &fix).unwrap();
    assert!(
        findings(&astro_report(&log), "astro.companion_destabilizes").is_empty(),
        "the suggested distance should actually be stable"
    );
}

#[test]
fn applying_the_demotion_also_clears_the_finding() {
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
    let r = astro_report(&log);
    let fix = findings(&r, "astro.companion_destabilizes")[0]
        .suggestions
        .iter()
        .find(|s| s.code.as_str() == "astro.demote_companion")
        .expect("the demotion fix")
        .clone();
    wb_constraints::apply_suggestion(&mut log, &fix).unwrap();
    assert!(
        findings(&astro_report(&log), "astro.companion_destabilizes").is_empty(),
        "demoting the companion below the stellar limit should actually clear the finding"
    );
}

#[test]
fn a_sub_stellar_companion_is_still_judged_on_its_period() {
    let mut log = earth_system();
    // Below the 0.08 solar-mass fusion limit, so it cannot destabilize anything as a
    // star — but 40 AU and a one-year period still contradict each other, and that is
    // arithmetic, not astrophysics.
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Brown Dwarf")),
            ("role", t("companion")),
            ("mass_solar", f(0.05)),
            ("distance_au", f(40.0)),
            ("period_years", f(1.0)),
        ],
    );
    let r = astro_report(&log);
    assert!(
        findings(&r, "astro.companion_destabilizes").is_empty(),
        "a sub-stellar object cannot destabilize the system as a star: {:?}",
        codes(&r)
    );
    assert_eq!(
        findings(&r, "astro.companion_period_mismatch").len(),
        1,
        "its stated period still contradicts its stated distance: {:?}",
        codes(&r)
    );
}

#[test]
fn moving_a_period_only_companion_out_does_not_raise_a_new_mismatch() {
    let mut log = earth_system();
    // Only a period is stated — no distance — so the fix has to add a distance beside
    // it, and the stale period has to go in the same transaction or `resolve` prefers
    // the new distance and immediately raises `astro.companion_period_mismatch`.
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Intruder")),
            ("role", t("companion")),
            ("mass_solar", f(0.8)),
            ("period_years", f(3.0)),
        ],
    );
    let r = astro_report(&log);
    let found = findings(&r, "astro.companion_destabilizes");
    assert_eq!(found.len(), 1, "{:?}", codes(&r));
    let fix = found[0]
        .suggestions
        .iter()
        .find(|s| s.code.as_str() == "astro.move_companion_out")
        .expect("the move-out fix")
        .clone();
    assert_eq!(
        fix.writes.len(),
        2,
        "distance_au plus clearing the stale period_years"
    );
    wb_constraints::apply_suggestion(&mut log, &fix).unwrap();
    let after = astro_report(&log);
    assert!(
        findings(&after, "astro.companion_destabilizes").is_empty(),
        "the suggested distance should actually be stable"
    );
    assert!(
        findings(&after, "astro.companion_period_mismatch").is_empty(),
        "the move-out fix must not trade one finding for another: {:?}",
        codes(&after)
    );
}

#[test]
fn a_distant_companion_leaves_the_planets_alone() {
    let mut log = earth_system();
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Far")),
            ("role", t("companion")),
            ("mass_solar", f(0.3)),
            ("distance_au", f(400.0)),
        ],
    );
    assert!(findings(&astro_report(&log), "astro.companion_destabilizes").is_empty());
}

#[test]
fn a_companion_whose_period_contradicts_its_distance_is_reported() {
    let mut log = earth_system();
    // "It returns every year" — entered as a one-local-year period — at 40 AU.
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
    let r = astro_report(&log);
    let found = findings(&r, "astro.companion_period_mismatch");
    assert_eq!(found.len(), 1, "{:?}", codes(&r));
    assert_eq!(found[0].badness, 0.8);
    // The finding states both numbers and the distance each implies.
    for key in [
        "stated_period_years",
        "implied_period_years",
        "stated_distance_au",
        "implied_distance_au",
    ] {
        assert!(found[0].params.contains_key(key), "missing {key}");
    }
    assert!(
        matches!(found[0].params.get("stated_period_years"), Some(Param::Float(y)) if (*y - 1.0).abs() < 1e-9)
    );
}

#[test]
fn a_consistent_companion_raises_no_mismatch() {
    let mut log = earth_system();
    // 40 AU about a 1.33-solar-mass pair is roughly 219 local years.
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Consistent")),
            ("role", t("companion")),
            ("mass_solar", f(0.33)),
            ("distance_au", f(40.0)),
            ("period_years", f(253.0)),
        ],
    );
    let report = astro_report(&log);
    let found = findings(&report, "astro.companion_period_mismatch");
    assert!(
        found.is_empty(),
        "253 years at 40 AU is within 25%: {found:?}"
    );
}

#[test]
fn a_period_only_companion_inside_the_system_is_reported() {
    let mut log = earth_system();
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Outer")),
            ("order", i(9)),
            ("class", t("ice_giant")),
            ("distance_au", f(30.0)),
        ],
    );
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Yearly")),
            ("role", t("companion")),
            ("mass_solar", f(0.33)),
            ("period_years", f(2.0)),
        ],
    );
    let report = astro_report(&log);
    let found = findings(&report, "astro.companion_period_mismatch");
    assert_eq!(found.len(), 1, "a 2-year orbit sits well inside Outer's");
    assert!(found[0].params.contains_key("implied_distance_au"));
}

#[test]
fn numbering_that_contradicts_the_distances_is_reported_and_renumbered() {
    let mut log = earth_system();
    // Two bodies numbered the wrong way round.
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
    let r = astro_report(&log);
    let found = findings(&r, "astro.order_mismatch");
    assert_eq!(found.len(), 2, "both are out of sequence: {:?}", codes(&r));
    for f in &found {
        assert!(f.badness > 0.0 && f.badness <= 1.0);
        assert_eq!(f.suggestions.len(), 1);
        assert_eq!(f.suggestions[0].code.as_str(), "astro.renumber");
        assert_eq!(f.suggestions[0].writes.len(), 1);
        assert_eq!(f.suggestions[0].writes[0].1, "order");
    }
    // Applying both renumberings clears the finding.
    let fixes: Vec<_> = found.iter().map(|f| f.suggestions[0].clone()).collect();
    for s in fixes {
        wb_constraints::apply_suggestion(&mut log, &s).unwrap();
    }
    assert!(findings(&astro_report(&log), "astro.order_mismatch").is_empty());
}

#[test]
fn badness_scales_with_how_far_out_of_sequence_a_body_is() {
    let mut log = earth_system();
    for (n, au) in [(2, 2.0), (3, 3.0), (4, 4.0), (5, 5.0), (6, 6.0)] {
        add(
            &mut log,
            "astro.body",
            vec![
                ("name", t(&format!("B{n}"))),
                ("order", i(n)),
                ("class", t("rocky")),
                ("distance_au", f(au)),
            ],
        );
    }
    assert!(findings(&astro_report(&log), "astro.order_mismatch").is_empty());
    // Move the outermost body to the front without renumbering it: five places out.
    let outer = named(&log, "B6");
    set(&mut log, outer, vec![("distance_au", f(0.1))]);
    let report = astro_report(&log);
    let found = findings(&report, "astro.order_mismatch");
    let worst = found.iter().map(|f| f.badness).fold(0.0_f64, f64::max);
    assert_eq!(worst, 1.0, "five places out of sequence saturates badness");
}

#[test]
fn crossing_orbits_are_reported_unless_a_resonance_protects_them() {
    let mut log = earth_system();
    // Two bodies whose eccentric orbits overlap, with an awkward period ratio.
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
    let r = astro_report(&log);
    let crossing = findings(&r, "astro.orbit_crossing");
    assert_eq!(crossing.len(), 1, "{:?}", codes(&r));
    assert_eq!(crossing[0].badness, 0.5);
    assert_eq!(crossing[0].related.len(), 1, "the other body is named");
    assert!(crossing[0].params.contains_key("other"));
}

#[test]
fn neptune_and_pluto_are_protected_by_their_resonance() {
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
    let r = astro_report(&log);
    assert!(
        findings(&r, "astro.orbit_crossing").is_empty(),
        "the 3:2 resonance should protect them: {:?}",
        codes(&r)
    );
    let protects = findings(&r, "astro.resonance_protects");
    assert_eq!(protects.len(), 1);
    assert_eq!(protects[0].kind, FindingKind::Support);
    assert_eq!(protects[0].badness, 0.0);
    // The finding is raised from Neptune, the lower id, so the ratio is its own period
    // over Pluto's: 2:3. The finding names both bodies, so it reads either way round.
    assert!(matches!(protects[0].params.get("p"), Some(Param::Int(2))));
    assert!(matches!(protects[0].params.get("q"), Some(Param::Int(3))));
}

#[test]
fn a_resonance_without_a_crossing_is_still_a_support() {
    let mut log = earth_system();
    // Circular 1:2 — no overlap, but worth saying out loud.
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
    let r = astro_report(&log);
    let res = findings(&r, "astro.resonance");
    assert_eq!(res.len(), 1, "{:?}", codes(&r));
    assert_eq!(res[0].kind, FindingKind::Support);
    assert!(findings(&r, "astro.orbit_crossing").is_empty());
}

#[test]
fn a_moon_outside_its_planets_hill_sphere_is_reported() {
    let mut log = earth_system();
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Runaway")),
            ("order", i(2)),
            ("class", t("moon")),
            ("parent", Value::Entity(EntityId::PLANET)),
            // Earth's Hill radius is 1.5 million km; a third of it is 500 000 km.
            ("distance_km", f(900_000.0)),
        ],
    );
    let r = astro_report(&log);
    let found = findings(&r, "astro.moon_outside_hill");
    assert_eq!(found.len(), 1, "{:?}", codes(&r));
    assert_eq!(found[0].badness, 0.9);
    assert!(found[0].params.contains_key("hill_third_km"));
    assert!(found[0].params.contains_key("distance_km"));
    // Luna, at 384 400 km, is fine.
    assert!(!found[0].related.contains(&named(&log, "Luna")));
}

#[test]
fn a_moon_inside_the_roche_limit_is_reported() {
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
    let r = astro_report(&log);
    let found = findings(&r, "astro.moon_inside_roche");
    assert_eq!(found.len(), 1, "{:?}", codes(&r));
    assert_eq!(found[0].badness, 0.9);
    assert!(found[0].params.contains_key("roche_km"));
}

#[test]
fn aethorias_three_moons_are_all_safe() {
    let r = astro_report(&aethoria());
    assert!(findings(&r, "astro.moon_outside_hill").is_empty());
    assert!(findings(&r, "astro.moon_inside_roche").is_empty());
}

#[test]
fn a_belt_lying_across_an_orbit_is_reported() {
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
    let r = astro_report(&log);
    let found = findings(&r, "astro.belt_overlaps_orbit");
    assert_eq!(found.len(), 1, "{:?}", codes(&r));
    assert_eq!(found[0].badness, 0.5);
    assert_eq!(found[0].related, vec![named(&log, "Resident")]);
    // The Veil, between Rubros and Goliath, overlaps nothing.
    assert!(findings(&astro_report(&aethoria()), "astro.belt_overlaps_orbit").is_empty());
}

#[test]
fn an_unreadable_system_produces_no_findings_and_no_panic() {
    // No primary star: SkyModel::build fails, so every checker must stay silent.
    let mut log = new_log();
    add_aethorian_calendar(&mut log);
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Lonely")),
            ("order", i(1)),
            ("class", t("rocky")),
            ("distance_au", f(1.0)),
        ],
    );
    let r = astro_report(&log);
    assert!(
        codes(&r).iter().all(|c| !c.starts_with("astro.")),
        "a system that cannot be built gets no astro findings: {:?}",
        codes(&r)
    );
    // The entities still get verdicts, just empty ones.
    assert!(r.verdicts.iter().any(|v| v.kind == "astro.body"));
}

#[test]
fn the_published_outer_criterion_gives_the_published_number() {
    // Holman & Wiegert 1999's P-type (circumbinary) critical distance. For the Aethorian
    // pair — mass ratio 0.33/1.53 and eccentricity 0.1 — it is 2.650 separations.
    close(critical_fraction_outer(0.33 / 1.53, 0.1), 2.650, 0.01);
    // A heavier or more eccentric pair pushes the stable circumbinary region further out.
    assert!(critical_fraction_outer(0.5, 0.4) > critical_fraction_outer(0.1, 0.0));
    assert!(critical_fraction_outer(f64::NAN, 0.1).is_finite());
}

#[test]
fn a_circumbinary_body_is_not_flagged_as_destabilized() {
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
    // Far outside the pair: circumbinary, and stable. The S-type criterion alone would
    // flag it, because its aphelion is past the inner critical distance.
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Outsider")),
            ("order", i(9)),
            ("class", t("ice_giant")),
            ("distance_au", f(60.0)),
        ],
    );
    let r = astro_report(&log);
    let found = findings(&r, "astro.companion_destabilizes");
    assert_eq!(found.len(), 1, "{:?}", codes(&r));
    assert!(
        !found[0].related.contains(&named(&log, "Outsider")),
        "a body orbiting well outside the pair is circumbinary, not at risk"
    );
    assert!(
        found[0].related.contains(&EntityId::PLANET),
        "the home world, inside the unstable annulus, still is"
    );
}

#[test]
fn moving_the_companion_out_clears_bodies_it_had_not_flagged() {
    // The regression this guards: `safe` must be derived from EVERY sibling's aphelion,
    // not only the flagged ones. Deriving it from the flagged subset moves the companion
    // to a distance whose widened inner critical radius re-exposes the outer bodies that
    // were previously stable, so the fix would stop clearing its own finding.
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
    add(
        &mut log,
        "astro.body",
        vec![
            ("name", t("Outsider")),
            ("order", i(9)),
            ("class", t("ice_giant")),
            ("distance_au", f(60.0)),
        ],
    );
    let r = astro_report(&log);
    let fix = findings(&r, "astro.companion_destabilizes")[0]
        .suggestions
        .iter()
        .find(|s| s.code.as_str() == "astro.move_companion_out")
        .expect("the move-out fix")
        .clone();
    wb_constraints::apply_suggestion(&mut log, &fix).unwrap();
    assert!(
        findings(&astro_report(&log), "astro.companion_destabilizes").is_empty(),
        "moving out must clear every body, including ones it had not flagged"
    );
}
