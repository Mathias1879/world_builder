use proptest::prelude::*;
use std::collections::BTreeSet;
use wb_constraints::{
    Finding, Grade, Param, Preset, Report, Status, effective_realism, grade, make_verdict,
};
use wb_editlog::{ActorId, EntityId, OpId};

fn ent(n: u32) -> EntityId {
    EntityId {
        op: OpId {
            lamport: 1,
            actor: ActorId([1; 16]),
        },
        n,
    }
}

#[test]
fn presets_and_thresholds() {
    assert_eq!(Preset::EarthStrict.value(), 0.0);
    assert_eq!(Preset::PlausibleFantasy.value(), 0.5);
    assert_eq!(Preset::HighFantasy.value(), 0.85);
    assert_eq!(grade(0.8, 0.0), Grade::Plausible);
    assert_eq!(grade(0.79, 0.0), Grade::Stretch);
    assert_eq!(grade(0.4, 0.0), Grade::Stretch);
    assert_eq!(grade(0.39, 0.0), Grade::Implausible);
    // 0.8 − 0.3·1.0 evaluates to 0.5000000000000001 in f64, so use a score clearly above it.
    assert_eq!(grade(0.51, 1.0), Grade::Plausible);
    assert_eq!(grade(0.09, 1.0), Grade::Implausible);
    assert_eq!(effective_realism(0.9, 0.5), 1.0);
    assert_eq!(effective_realism(0.2, -0.5), 0.0);
}

#[test]
fn verdict_scores_worst_unkept_issue() {
    let findings = vec![
        Finding::issue("c", "a.minor", 0.2),
        Finding::issue("c", "a.major", 0.7),
        Finding::support("c", "a.ok"),
        Finding::consequence("c", "a.effect"),
    ];
    let v = make_verdict(
        ent(0),
        "feature.desert",
        findings.clone(),
        &BTreeSet::new(),
        0.0,
    );
    assert!((v.score - 0.3).abs() < 1e-12);
    assert_eq!(v.grade, Grade::Implausible);
    assert_eq!(
        (v.issues.len(), v.supports.len(), v.consequences.len()),
        (2, 1, 1)
    );
    assert_eq!(v.status, Status::Open);

    let keep: BTreeSet<String> = ["a.major".to_string()].into_iter().collect();
    let v = make_verdict(ent(0), "feature.desert", findings.clone(), &keep, 0.0);
    assert!((v.score - 0.8).abs() < 1e-12);
    assert_eq!(v.grade, Grade::Plausible);
    assert_eq!(v.intentional.len(), 1);
    assert_eq!(v.status, Status::Open, "a.minor is still open");

    let keep_all: BTreeSet<String> = ["a.major".to_string(), "a.minor".to_string()]
        .into_iter()
        .collect();
    let v = make_verdict(ent(0), "feature.desert", findings, &keep_all, 0.0);
    assert_eq!((v.status, v.score), (Status::Intentional, 1.0));
}

#[test]
fn report_counts_and_hash() {
    let a = make_verdict(ent(0), "k", vec![], &BTreeSet::new(), 0.5);
    let b = make_verdict(
        ent(1),
        "k",
        vec![Finding::issue("c", "x", 0.5)],
        &BTreeSet::new(),
        0.5,
    );
    let keep: BTreeSet<String> = ["x".to_string()].into_iter().collect();
    let c = make_verdict(ent(2), "k", vec![Finding::issue("c", "x", 0.9)], &keep, 0.5);
    let r = Report::new(vec![a.clone(), b.clone(), c.clone()]);
    assert_eq!(
        (
            r.counts.plausible,
            r.counts.stretch,
            r.counts.implausible,
            r.counts.intentional
        ),
        (1, 1, 0, 1)
    );
    assert_eq!(r.hash, Report::new(vec![a.clone(), b.clone(), c]).hash);
    assert_ne!(r.hash, Report::new(vec![a, b]).hash);
}

#[test]
fn finding_builders() {
    let f = Finding::issue("static.lore_area", "lore.area_mismatch", 0.4)
        .with_param("ratio", Param::Float(2.0))
        .with_related(ent(7));
    assert_eq!(f.checker, "static.lore_area");
    assert_eq!(f.code.as_str(), "lore.area_mismatch");
    assert_eq!(f.params["ratio"], Param::Float(2.0));
    assert_eq!(f.related, vec![ent(7)]);
}

fn cfg() -> ProptestConfig {
    ProptestConfig {
        cases: 256,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

fn rank(g: Grade) -> u8 {
    match g {
        Grade::Plausible => 0,
        Grade::Stretch => 1,
        Grade::Implausible => 2,
    }
}

proptest! {
    #![proptest_config(cfg())]
    #[test]
    fn grade_never_harsher_as_realism_rises(score in 0.0f64..=1.0, r1 in 0.0f64..=1.0, r2 in 0.0f64..=1.0) {
        let (lo, hi) = if r1 <= r2 { (r1, r2) } else { (r2, r1) };
        prop_assert!(rank(grade(score, hi)) <= rank(grade(score, lo)));
    }

    #[test]
    fn grade_never_harsher_as_score_rises(s1 in 0.0f64..=1.0, s2 in 0.0f64..=1.0, r in 0.0f64..=1.0) {
        let (lo, hi) = if s1 <= s2 { (s1, s2) } else { (s2, s1) };
        prop_assert!(rank(grade(hi, r)) <= rank(grade(lo, r)));
    }
}
