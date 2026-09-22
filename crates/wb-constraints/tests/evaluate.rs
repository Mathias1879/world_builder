mod common;

use common::{add, new_log, poly};
use wb_constraints::{
    CheckContext, Checker, CheckerRegistry, ConstraintError, ConstraintView, Finding, Grade,
    KindPattern, Param, Status, UnknownTerrain, evaluate, realism_of, render,
};
use wb_editlog::{EntityId, Value};
use wb_world::Planet;

/// Flags every desert with a fixed badness; supports every forest.
struct Probe(f64);

impl Checker for Probe {
    fn id(&self) -> &'static str {
        "test.probe"
    }
    fn kinds(&self) -> &[KindPattern] {
        &[
            KindPattern::Exact("feature.desert"),
            KindPattern::Prefix("feature.for"),
        ]
    }
    fn check(&self, c: ConstraintView<'_>, _ctx: &CheckContext<'_>) -> Vec<Finding> {
        match c.kind() {
            Some("feature.desert") => vec![
                Finding::issue("test.probe", "test.too_dry", self.0)
                    .with_param("pct", Param::Int(40)),
            ],
            _ => vec![Finding::support("test.probe", "test.fine")],
        }
    }
}

fn sq() -> Value {
    poly(&[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)])
}

#[test]
fn kind_patterns() {
    assert!(KindPattern::Exact("lore.area").matches("lore.area"));
    assert!(!KindPattern::Exact("lore.area").matches("lore.area2"));
    assert!(KindPattern::Prefix("feature.").matches("feature.lake"));
    assert!(!KindPattern::Prefix("feature.").matches("civ.settlement"));
}

#[test]
fn registry_rejects_duplicates() {
    let mut r = CheckerRegistry::new();
    r.register(Box::new(Probe(0.5))).unwrap();
    assert_eq!(
        r.register(Box::new(Probe(0.1))).unwrap_err(),
        ConstraintError::DuplicateChecker("test.probe".into())
    );
    assert_eq!(r.ids(), vec!["test.probe"]);
}

#[test]
fn evaluation_grades_keeps_and_counts() {
    let mut log = new_log();
    let desert = add(&mut log, "feature.desert", vec![("area", sq())]);
    let forest = add(&mut log, "feature.forest", vec![("area", sq())]);
    add(&mut log, "import.base_map", vec![]); // not a constraint
    let mut reg = CheckerRegistry::new();
    reg.register(Box::new(Probe(0.7))).unwrap();

    assert_eq!(realism_of(log.state()), 0.5);
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let report = evaluate(&ctx, &reg);
    assert_eq!(report.verdicts.len(), 2);
    let dv = &report.verdicts[0];
    assert_eq!(dv.constraint, desert);
    assert!((dv.score - 0.3).abs() < 1e-12);
    assert_eq!(dv.grade, Grade::Stretch, "r = 0.5: 0.25 ≤ 0.3 < 0.65");
    assert_eq!(report.verdicts[1].constraint, forest);
    assert_eq!(report.verdicts[1].supports.len(), 1);
    assert_eq!((report.counts.plausible, report.counts.stretch), (1, 1));

    let mut tx = log.transact("tolerant + kept");
    tx.set(desert, "tolerance", -0.5);
    tx.commit().unwrap();
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    assert_eq!(
        evaluate(&ctx, &reg).verdicts[0].grade,
        Grade::Implausible,
        "r = 0: 0.3 < 0.4"
    );

    let mut tx = log.transact("keep");
    tx.set(
        desert,
        "keep_codes",
        Value::List(vec![Value::from("test.too_dry")]),
    );
    tx.commit().unwrap();
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let r = evaluate(&ctx, &reg);
    assert_eq!(r.verdicts[0].status, Status::Intentional);
    assert_eq!(r.counts.intentional, 1);

    let mut tx = log.transact("realism");
    tx.set(EntityId::PLANET, "realism", 0.85);
    tx.commit().unwrap();
    assert_eq!(realism_of(log.state()), 0.85);
}

#[test]
fn evaluation_is_deterministic() {
    let mut log = new_log();
    add(&mut log, "feature.desert", vec![("area", sq())]);
    let mut reg = CheckerRegistry::new();
    reg.register(Box::new(Probe(0.4))).unwrap();
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    assert_eq!(evaluate(&ctx, &reg).hash, evaluate(&ctx, &reg).hash);
}

#[test]
fn rendering() {
    let f = Finding::issue("static.lore_area", "lore.area_mismatch", 0.5)
        .with_param("drawn", Param::Float(1_234_567.8))
        .with_param("stated", Param::Float(600_000.0))
        .with_param("unit", Param::Text("mi2".into()))
        .with_param("ratio", Param::Float(2.057));
    assert_eq!(
        render(&f),
        "Drawn area is 1234568 mi2 but the lore says 600000 mi2 (2.06× off)."
    );
    let unknown = Finding::issue("x", "made.up", 0.1).with_param("n", Param::Int(3));
    assert_eq!(render(&unknown), "made.up n=3");
}
