# Constraints & Feasibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build subsystem [3] — `wb-constraints`: typed constraints over Edit Log entities, a registry of pure checkers, graded explainable verdicts with "keep anyway" and one-click suggestions, and six starter checks that need no simulation.

**Architecture:** Constraints are live Edit Log entities of catalogued kinds. Registered `Checker`s turn each constraint (plus a read-only world context) into findings; the framework grades them against the realism dial, applies keep-anyway codes, and emits a deterministic `Report`. Actions (apply suggestion, keep anyway, set realism) are ordinary undoable Edit Log transactions.

**Tech Stack:** Rust 1.90.0 (edition 2024), `serde` + `postcard`, `blake3`, `libm`, `proptest`; crates `wb-grid`, `wb-world`, `wb-editlog`.

**Spec:** `docs/superpowers/specs/2026-09-22-constraints-design.md` (parent: `docs/superpowers/specs/2026-09-21-world-builder-architecture-design.md` §2.3, §5).

## Global Constraints

- Phase-1 determinism rules hold (`clippy.toml`): transcendental math only via `libm::`; no `mul_add`; no `HashMap`/`HashSet`; no `unsafe`; no randomness.
- Checkers are pure and never panic; malformed or missing input yields no finding. Framework errors are typed `ConstraintError`.
- Grade: Plausible if `score ≥ 0.8 − 0.3·r`; Implausible if `score < 0.4 − 0.3·r`; else Stretch. `r = clamp(slider + tolerance, 0, 1)`. Presets: EarthStrict 0.0, PlausibleFantasy 0.5, HighFantasy 0.85; absent `realism` on the planet means 0.5.
- Score = `1 − max(badness of issues whose code is not kept)`, 1.0 with none. Status Intentional iff ≥ 1 issue and all issues kept.
- `Report::hash = blake3("wb-report-v1" ‖ postcard(verdicts))` must be identical on aarch64, wasm32-wasip1, x86-64 (checked by `scripts/ci-local.sh`).
- mi² → km² factor `2.589988110336`.
- All commands run from the repository root (`~/dev/world_builder`, or the plan's worktree). Rust build output stays on the external drive (`/Volumes/Untitled/dev`) — never change target-dir settings.
- Run `cargo fmt --all` before every `scripts/check.sh`; the gate must be green before each commit.
- Commits end with exactly `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.

## File Structure

```
crates/wb-editlog/src/value.rs          + Value::Entity(EntityId) (last variant) and From<EntityId>
crates/wb-constraints/
  Cargo.toml
  src/lib.rs            re-exports
  src/error.rs          ConstraintError
  src/model.rs          IssueCode, FindingKind, Param, Finding, Suggestion, Grade, Status, Preset, Verdict, Counts, Report
  src/grade.rs          effective_realism, grade, make_verdict
  src/geo.rs            centroid, midpoint, samples, contains (spherical winding test)
  src/kinds.rs          kind catalog, validators (register_kinds), field accessors
  src/checker.rs        Checker, KindPattern, Terrain, UnknownTerrain, CheckContext, CheckerRegistry
  src/evaluate.rs       realism_of, evaluate
  src/templates.rs      render, render_suggestion
  src/actions.rs        apply_suggestion, keep_anyway, unkeep, set_realism
  src/checks/mod.rs     CheckerRegistry::with_starter_checks
  src/checks/coast.rs  src/checks/river_mouth.rs  src/checks/lore_area.rs
  src/checks/relative_position.rs  src/checks/region_rules.rs  src/checks/latitude.rs
  tests/common/mod.rs   log builder, geometry helpers, fake terrain
  tests/model.rs  tests/geo.rs  tests/kinds.rs  tests/evaluate.rs  tests/actions.rs
  tests/checks_terrain.rs  tests/checks_lore.rs  tests/checks_rules.rs
  tests/aethoria.rs  tests/golden.rs  tests/perf.rs
scripts/check.sh        + release perf guard (native + wasm)
```

---

### Task 1: Entity references, crate scaffold, model and grading

**Files:**
- Modify: `crates/wb-editlog/src/value.rs`
- Create: `crates/wb-constraints/Cargo.toml`, `src/lib.rs`, `src/error.rs`, `src/model.rs`, `src/grade.rs`
- Test: `crates/wb-editlog/tests/value.rs` (append), `crates/wb-constraints/tests/model.rs`

**Interfaces:**
- Consumes: `wb_editlog::{EntityId, Value, EditError}`.
- Produces:
  - `Value::Entity(EntityId)` (last variant) + `impl From<EntityId> for Value`.
  - `ConstraintError { DuplicateChecker(String), UnknownEntity(EntityId), NotAConstraint(EntityId), Edit(EditError) }` + `From<EditError>`, `Display`, `Error`, `Clone, Debug, PartialEq`.
  - `IssueCode(pub String)` with `new(&str)`, `as_str()`, `From<&str>`; `FindingKind { Support, Issue, Consequence }`; `Param { Int(i64), Float(f64), Text(String), Entity(EntityId) }`; `Suggestion { code: IssueCode, params: BTreeMap<String, Param>, writes: Vec<(EntityId, String, Value)> }`; `Finding { checker: String, kind: FindingKind, code: IssueCode, badness: f64, params: BTreeMap<String, Param>, related: Vec<EntityId>, suggestions: Vec<Suggestion> }` with constructors `Finding::issue(checker, code, badness)`, `Finding::support(checker, code)`, `Finding::consequence(checker, code)` and builders `with_param(k, Param)`, `with_related(EntityId)`, `with_suggestion(Suggestion)`.
  - `Grade { Plausible, Stretch, Implausible }`, `Status { Open, Intentional }`, `Preset { EarthStrict, PlausibleFantasy, HighFantasy }` with `value() -> f64`.
  - `Verdict { constraint, kind: String, grade, score: f64, status, issues, intentional, supports, consequences: Vec<Finding> }`; `Counts { plausible, stretch, implausible, intentional: u32 }`; `Report { verdicts: Vec<Verdict>, counts: Counts, hash: [u8; 32] }` with `Report::new(Vec<Verdict>) -> Report`. All model types derive `Clone, Debug, PartialEq, Serialize, Deserialize` (enums also `Copy, Eq` where fields allow).
  - `effective_realism(slider: f64, tolerance: f64) -> f64`; `grade(score: f64, r: f64) -> Grade`; `make_verdict(constraint: EntityId, kind: &str, findings: Vec<Finding>, keep: &BTreeSet<String>, r: f64) -> Verdict`.
  - Counting rule: a verdict with `Status::Intentional` counts only in `intentional`; others count by grade.

- [ ] **Step 1: Failing test for `Value::Entity`**

Append to `crates/wb-editlog/tests/value.rs`:
```rust
#[test]
fn entity_references_are_values() {
    let e = wb_editlog::EntityId { op: wb_editlog::OpId { lamport: 3, actor: wb_editlog::ActorId([2; 16]) }, n: 1 };
    let v = Value::from(e);
    assert_eq!(v, Value::Entity(e));
    assert_eq!(v.clone().canonical("subject").unwrap(), v);
    let bytes = postcard::to_allocvec(&v).unwrap();
    assert_eq!(postcard::from_bytes::<Value>(&bytes).unwrap(), v);
}
```
Run: `cargo test -p wb-editlog --test value` — expected FAIL (`no variant named Entity`).

- [ ] **Step 2: Add the variant**

In `crates/wb-editlog/src/value.rs`, add `Entity(EntityId)` as the **last** variant of `Value` (after `List(...)`), import `crate::ids::EntityId`, and add:
```rust
impl From<EntityId> for Value {
    fn from(v: EntityId) -> Self {
        Value::Entity(v)
    }
}
```
If any `match` on `Value` in wb-editlog becomes non-exhaustive, add `Value::Entity(_)` to the arm that passes values through unchanged.

Run: `cargo test -p wb-editlog` — expected all PASS, **including `golden_edit_log` unchanged** (adding a last variant does not change existing encodings).

- [ ] **Step 3: Crate manifest, lib, errors**

`crates/wb-constraints/Cargo.toml`:
```toml
[package]
name = "wb-constraints"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
wb-grid = { workspace = true, features = ["serde"] }
wb-world = { workspace = true, features = ["serde"] }
wb-editlog = { path = "../wb-editlog" }
libm.workspace = true
blake3.workspace = true
serde.workspace = true
postcard.workspace = true

[dev-dependencies]
proptest.workspace = true

[lints]
workspace = true
```

`crates/wb-constraints/src/lib.rs`:
```rust
//! Constraints & Feasibility: checkers, graded verdicts, keep-anyway, suggestions.

mod error;
mod grade;
mod model;

pub use error::ConstraintError;
pub use grade::{effective_realism, grade, make_verdict};
pub use model::{Counts, Finding, FindingKind, Grade, IssueCode, Param, Preset, Report, Status, Suggestion, Verdict};
```

`crates/wb-constraints/src/error.rs`:
```rust
use core::fmt;
use wb_editlog::{EditError, EntityId};

#[derive(Clone, Debug, PartialEq)]
pub enum ConstraintError {
    DuplicateChecker(String),
    UnknownEntity(EntityId),
    NotAConstraint(EntityId),
    Edit(EditError),
}

impl From<EditError> for ConstraintError {
    fn from(e: EditError) -> Self {
        ConstraintError::Edit(e)
    }
}

impl fmt::Display for ConstraintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConstraintError::DuplicateChecker(id) => write!(f, "checker '{id}' is already registered"),
            ConstraintError::UnknownEntity(e) => write!(f, "unknown entity {e:?}"),
            ConstraintError::NotAConstraint(e) => write!(f, "entity {e:?} is not a constraint"),
            ConstraintError::Edit(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ConstraintError {}
```

- [ ] **Step 4: Failing model/grading tests**

`crates/wb-constraints/tests/model.rs`:
```rust
use proptest::prelude::*;
use std::collections::BTreeSet;
use wb_constraints::{Finding, Grade, Param, Preset, Report, Status, effective_realism, grade, make_verdict};
use wb_editlog::{ActorId, EntityId, OpId};

fn ent(n: u32) -> EntityId {
    EntityId { op: OpId { lamport: 1, actor: ActorId([1; 16]) }, n }
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
    let v = make_verdict(ent(0), "feature.desert", findings.clone(), &BTreeSet::new(), 0.0);
    assert!((v.score - 0.3).abs() < 1e-12);
    assert_eq!(v.grade, Grade::Implausible);
    assert_eq!((v.issues.len(), v.supports.len(), v.consequences.len()), (2, 1, 1));
    assert_eq!(v.status, Status::Open);

    let keep: BTreeSet<String> = ["a.major".to_string()].into_iter().collect();
    let v = make_verdict(ent(0), "feature.desert", findings.clone(), &keep, 0.0);
    assert!((v.score - 0.8).abs() < 1e-12);
    assert_eq!(v.grade, Grade::Plausible);
    assert_eq!(v.intentional.len(), 1);
    assert_eq!(v.status, Status::Open, "a.minor is still open");

    let keep_all: BTreeSet<String> = ["a.major".to_string(), "a.minor".to_string()].into_iter().collect();
    let v = make_verdict(ent(0), "feature.desert", findings, &keep_all, 0.0);
    assert_eq!((v.status, v.score), (Status::Intentional, 1.0));
}

#[test]
fn report_counts_and_hash() {
    let a = make_verdict(ent(0), "k", vec![], &BTreeSet::new(), 0.5);
    let b = make_verdict(ent(1), "k", vec![Finding::issue("c", "x", 0.5)], &BTreeSet::new(), 0.5);
    let keep: BTreeSet<String> = ["x".to_string()].into_iter().collect();
    let c = make_verdict(ent(2), "k", vec![Finding::issue("c", "x", 0.9)], &keep, 0.5);
    let r = Report::new(vec![a.clone(), b.clone(), c.clone()]);
    assert_eq!((r.counts.plausible, r.counts.stretch, r.counts.implausible, r.counts.intentional), (1, 1, 0, 1));
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
    ProptestConfig { cases: 256, failure_persistence: None, ..ProptestConfig::default() }
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
```

Run: `cargo test -p wb-constraints --test model` — expected FAIL (unresolved modules).

- [ ] **Step 5: Implement model and grading**

`crates/wb-constraints/src/model.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use wb_editlog::{EntityId, Value};

/// Dotted issue identifier, e.g. `lore.area_mismatch`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct IssueCode(pub String);

impl IssueCode {
    pub fn new(s: &str) -> Self {
        IssueCode(s.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for IssueCode {
    fn from(s: &str) -> Self {
        IssueCode::new(s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FindingKind {
    Support,
    Issue,
    Consequence,
}

/// Explanation parameter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Param {
    Int(i64),
    Float(f64),
    Text(String),
    Entity(EntityId),
}

/// A one-click fix: field writes committed as one Edit Log transaction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Suggestion {
    pub code: IssueCode,
    pub params: BTreeMap<String, Param>,
    pub writes: Vec<(EntityId, String, Value)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub checker: String,
    pub kind: FindingKind,
    pub code: IssueCode,
    /// Issue badness in [0, 1]; 0 for supports and consequences.
    pub badness: f64,
    pub params: BTreeMap<String, Param>,
    pub related: Vec<EntityId>,
    pub suggestions: Vec<Suggestion>,
}

impl Finding {
    fn new(checker: &str, kind: FindingKind, code: &str, badness: f64) -> Self {
        Finding {
            checker: checker.to_string(),
            kind,
            code: IssueCode::new(code),
            badness,
            params: BTreeMap::new(),
            related: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn issue(checker: &str, code: &str, badness: f64) -> Self {
        Finding::new(checker, FindingKind::Issue, code, badness)
    }

    pub fn support(checker: &str, code: &str) -> Self {
        Finding::new(checker, FindingKind::Support, code, 0.0)
    }

    pub fn consequence(checker: &str, code: &str) -> Self {
        Finding::new(checker, FindingKind::Consequence, code, 0.0)
    }

    pub fn with_param(mut self, key: &str, value: Param) -> Self {
        self.params.insert(key.to_string(), value);
        self
    }

    pub fn with_related(mut self, e: EntityId) -> Self {
        self.related.push(e);
        self
    }

    pub fn with_suggestion(mut self, s: Suggestion) -> Self {
        self.suggestions.push(s);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Grade {
    Plausible,
    Stretch,
    Implausible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Open,
    Intentional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    EarthStrict,
    PlausibleFantasy,
    HighFantasy,
}

impl Preset {
    pub fn value(self) -> f64 {
        match self {
            Preset::EarthStrict => 0.0,
            Preset::PlausibleFantasy => 0.5,
            Preset::HighFantasy => 0.85,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub constraint: EntityId,
    pub kind: String,
    pub grade: Grade,
    pub score: f64,
    pub status: Status,
    pub issues: Vec<Finding>,
    pub intentional: Vec<Finding>,
    pub supports: Vec<Finding>,
    pub consequences: Vec<Finding>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub plausible: u32,
    pub stretch: u32,
    pub implausible: u32,
    pub intentional: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub verdicts: Vec<Verdict>,
    pub counts: Counts,
    pub hash: [u8; 32],
}

impl Report {
    pub fn new(verdicts: Vec<Verdict>) -> Report {
        let mut counts = Counts::default();
        for v in &verdicts {
            match (v.status, v.grade) {
                (Status::Intentional, _) => counts.intentional += 1,
                (Status::Open, Grade::Plausible) => counts.plausible += 1,
                (Status::Open, Grade::Stretch) => counts.stretch += 1,
                (Status::Open, Grade::Implausible) => counts.implausible += 1,
            }
        }
        let mut h = blake3::Hasher::new();
        h.update(b"wb-report-v1");
        h.update(&postcard::to_allocvec(&verdicts).expect("verdicts always serialize"));
        Report { verdicts, counts, hash: h.finalize().into() }
    }
}
```

`crates/wb-constraints/src/grade.rs`:
```rust
use crate::model::{Finding, FindingKind, Grade, Status, Verdict};
use std::collections::BTreeSet;
use wb_editlog::EntityId;

/// `clamp(slider + tolerance, 0, 1)`.
pub fn effective_realism(slider: f64, tolerance: f64) -> f64 {
    (slider + tolerance).clamp(0.0, 1.0)
}

/// Plausible if `score ≥ 0.8 − 0.3r`, Implausible if `score < 0.4 − 0.3r`, else Stretch.
pub fn grade(score: f64, r: f64) -> Grade {
    if score >= 0.8 - 0.3 * r {
        Grade::Plausible
    } else if score < 0.4 - 0.3 * r {
        Grade::Implausible
    } else {
        Grade::Stretch
    }
}

/// Partitions findings, applies kept codes, and grades one constraint.
pub fn make_verdict(constraint: EntityId, kind: &str, findings: Vec<Finding>, keep: &BTreeSet<String>, r: f64) -> Verdict {
    let mut issues = Vec::new();
    let mut intentional = Vec::new();
    let mut supports = Vec::new();
    let mut consequences = Vec::new();
    for f in findings {
        match f.kind {
            FindingKind::Support => supports.push(f),
            FindingKind::Consequence => consequences.push(f),
            FindingKind::Issue if keep.contains(f.code.as_str()) => intentional.push(f),
            FindingKind::Issue => issues.push(f),
        }
    }
    let score = 1.0 - issues.iter().map(|f| f.badness).fold(0.0, f64::max);
    let status = if issues.is_empty() && !intentional.is_empty() { Status::Intentional } else { Status::Open };
    Verdict {
        constraint,
        kind: kind.to_string(),
        grade: grade(score, r),
        score,
        status,
        issues,
        intentional,
        supports,
        consequences,
    }
}
```

- [ ] **Step 6: Run, gate, commit**

Run: `cargo test -p wb-constraints --test model` — expected PASS (6 tests).
Run: `cargo fmt --all && scripts/check.sh` — expected green.
```bash
git add Cargo.lock crates/wb-editlog crates/wb-constraints
git commit -m "feat(constraints): crate scaffold, verdict model and grading; Value::Entity in editlog" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: Spherical geometry helpers

**Files:**
- Create: `crates/wb-constraints/src/geo.rs`
- Modify: `crates/wb-constraints/src/lib.rs`
- Test: `crates/wb-constraints/tests/geo.rs`

**Interfaces:**
- Consumes: `wb_grid::{LatLon, Vec3}`.
- Produces: `centroid(&[LatLon]) -> Option<LatLon>` (normalized sum of unit vectors; `None` if empty or degenerate), `midpoint(LatLon, LatLon) -> LatLon` (great-circle), `samples(&[LatLon], closed: bool) -> Vec<LatLon>` (vertices then segment midpoints, plus the closing edge's midpoint when `closed` and ≥ 3 points), `contains(ring: &[LatLon], p: LatLon) -> bool` (winding test; ring smaller than a hemisphere).

- [ ] **Step 1: Failing tests**

`crates/wb-constraints/tests/geo.rs`:
```rust
use wb_constraints::{centroid, contains, midpoint, samples};
use wb_grid::LatLon;

fn d(lat: f64, lon: f64) -> LatLon {
    LatLon::from_degrees(lat, lon)
}

fn deg(p: LatLon) -> (f64, f64) {
    (p.lat.to_degrees(), p.lon.to_degrees())
}

#[test]
fn centroid_and_midpoint() {
    let (lat, lon) = deg(centroid(&[d(0.0, -10.0), d(0.0, 10.0)]).unwrap());
    assert!(lat.abs() < 1e-9 && lon.abs() < 1e-9);
    let (lat, _) = deg(midpoint(d(10.0, 0.0), d(30.0, 0.0)));
    assert!((lat - 20.0).abs() < 1e-9);
    assert!(centroid(&[]).is_none());
}

#[test]
fn samples_include_midpoints() {
    let ring = [d(0.0, 0.0), d(0.0, 10.0), d(10.0, 10.0)];
    assert_eq!(samples(&ring, false).len(), 5);
    assert_eq!(samples(&ring, true).len(), 6);
    assert_eq!(samples(&[d(1.0, 1.0)], false).len(), 1);
}

#[test]
fn point_in_simple_polygon() {
    let square = [d(-10.0, -10.0), d(-10.0, 10.0), d(10.0, 10.0), d(10.0, -10.0)];
    assert!(contains(&square, d(0.0, 0.0)));
    assert!(!contains(&square, d(20.0, 0.0)));
    assert!(!contains(&square, d(0.0, 170.0)));
    let reversed: Vec<LatLon> = square.iter().rev().copied().collect();
    assert!(contains(&reversed, d(0.0, 0.0)), "orientation does not matter");
}

#[test]
fn polygon_across_the_antimeridian() {
    let ring = [d(-5.0, 170.0), d(-5.0, -170.0), d(5.0, -170.0), d(5.0, 170.0)];
    assert!(contains(&ring, d(0.0, 180.0)));
    assert!(contains(&ring, d(0.0, -175.0)));
    assert!(!contains(&ring, d(0.0, 0.0)));
}

#[test]
fn polygon_around_a_pole() {
    let ring: Vec<LatLon> = (0..36).map(|i| d(80.0, -180.0 + 10.0 * i as f64)).collect();
    assert!(contains(&ring, d(90.0, 0.0)));
    assert!(contains(&ring, d(85.0, 123.0)));
    assert!(!contains(&ring, d(70.0, 0.0)));
}
```
Run: `cargo test -p wb-constraints --test geo` — expected FAIL (unresolved imports).

- [ ] **Step 2: Implement**

`crates/wb-constraints/src/geo.rs`:
```rust
use core::f64::consts::PI;
use wb_grid::{LatLon, Vec3};

/// Spherical centroid: normalized sum of unit vectors. `None` if empty or degenerate.
pub fn centroid(points: &[LatLon]) -> Option<LatLon> {
    let sum = points.iter().fold(Vec3::new(0.0, 0.0, 0.0), |acc, p| acc.plus(p.to_vec3()));
    if points.is_empty() || sum.length() < 1e-12 {
        return None;
    }
    Some(LatLon::from_vec3(sum.normalize()))
}

/// Great-circle midpoint.
pub fn midpoint(a: LatLon, b: LatLon) -> LatLon {
    LatLon::from_vec3(a.to_vec3().plus(b.to_vec3()).normalize())
}

/// Vertices followed by segment midpoints (and the closing edge's midpoint if `closed`).
pub fn samples(points: &[LatLon], closed: bool) -> Vec<LatLon> {
    let mut out = points.to_vec();
    for w in points.windows(2) {
        out.push(midpoint(w[0], w[1]));
    }
    if closed && points.len() > 2 {
        out.push(midpoint(points[points.len() - 1], points[0]));
    }
    out
}

/// Winding test on the sphere: sum the signed angles each edge subtends around `p`
/// (edges projected onto the tangent plane at `p`); inside when |sum| > π.
pub fn contains(ring: &[LatLon], p: LatLon) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let pv = p.to_vec3();
    let tangent = |q: LatLon| {
        let v = q.to_vec3();
        v.plus(pv.scaled(-v.dot(pv)))
    };
    let mut total = 0.0;
    for (i, q) in ring.iter().enumerate() {
        let a = tangent(*q);
        let b = tangent(ring[(i + 1) % ring.len()]);
        total += libm::atan2(pv.dot(a.cross(b)), a.dot(b));
    }
    total.abs() > PI
}
```

Add to `src/lib.rs`:
```rust
mod geo;
pub use geo::{centroid, contains, midpoint, samples};
```

- [ ] **Step 3: Run, gate, commit**

Run: `cargo test -p wb-constraints --test geo` — expected PASS (5 tests).
Run: `cargo fmt --all && scripts/check.sh` — expected green.
```bash
git add crates/wb-constraints
git commit -m "feat(constraints): spherical centroid, midpoints, samples, point-in-polygon" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: Kind catalog, validators, field accessors, test helpers

**Files:**
- Create: `crates/wb-constraints/src/kinds.rs`, `crates/wb-constraints/tests/common/mod.rs`
- Modify: `crates/wb-constraints/src/lib.rs`
- Test: `crates/wb-constraints/tests/kinds.rs`

**Interfaces:**
- Consumes: `wb_editlog::{EditLog, EntityKind, EntityView, Value}`, `wb_world::Geometry`.
- Produces:
  - `pub const CONSTRAINT_KINDS: &[&str]` (11 kinds, spec §6), `is_constraint_kind(&str) -> bool`, `register_kinds(&mut EditLog)`.
  - Accessors (all `pub`, all return `None` on missing/wrong type): `polygon<'a>(e: &EntityView<'a>, field: &str) -> Option<&'a [LatLon]>`, `line<'a>(e, field) -> Option<&'a [LatLon]>`, `point(e, field) -> Option<LatLon>`, `text<'a>(e, field) -> Option<&'a str>`, `float(e, field) -> Option<f64>`, `entity(e, field) -> Option<EntityId>`, `shape(e: &EntityView<'_>) -> Option<(Vec<LatLon>, bool)>` (points and `closed`: `area` polygon → closed; else `spine`/`path` line → open; else `at` point → open).
  - Test helpers in `tests/common/mod.rs`: `new_log() -> EditLog` (kinds registered), `add(&mut EditLog, kind, Vec<(&str, Value)>) -> EntityId`, `d(lat, lon) -> LatLon`, `poly(&[(f64, f64)]) -> Value`, `line(&[(f64, f64)]) -> Value`, `band(s, n, w, e) -> Value` (lat/lon box polygon with a vertex every 1° along each edge), `WestIsLand` terrain (land where lon < 0).

- [ ] **Step 1: Test helpers**

`crates/wb-constraints/tests/common/mod.rs`:
```rust
#![allow(dead_code)]
use wb_constraints::{Terrain, register_kinds};
use wb_editlog::{ActorId, AuthorId, EditLog, EntityId, FixedClock, Value};
use wb_grid::LatLon;
use wb_world::Geometry;

pub fn new_log() -> EditLog {
    let mut log = EditLog::new(ActorId([1; 16]), AuthorId([9; 16]), Box::new(FixedClock(1_758_000_000_000)));
    register_kinds(&mut log);
    log
}

pub fn add(log: &mut EditLog, kind: &str, fields: Vec<(&str, Value)>) -> EntityId {
    let mut tx = log.transact("add");
    let e = tx.create(kind, fields);
    tx.commit().unwrap();
    e
}

pub fn d(lat: f64, lon: f64) -> LatLon {
    LatLon::from_degrees(lat, lon)
}

pub fn poly(pts: &[(f64, f64)]) -> Value {
    Value::Geometry(Geometry::Polygon(pts.iter().map(|&(a, b)| d(a, b)).collect()))
}

pub fn line(pts: &[(f64, f64)]) -> Value {
    Value::Geometry(Geometry::LineString(pts.iter().map(|&(a, b)| d(a, b)).collect()))
}

/// Lat/lon box as a polygon with a vertex every 1° along each edge (close to a true lat/lon box).
pub fn band(s: f64, n: f64, w: f64, e: f64) -> Value {
    let mut pts = Vec::new();
    let steps = |a: f64, b: f64| ((b - a).abs().ceil() as usize).max(1);
    let (ns, nw) = (steps(s, n), steps(w, e));
    for i in 0..nw {
        pts.push((s, w + (e - w) * i as f64 / nw as f64));
    }
    for i in 0..ns {
        pts.push((s + (n - s) * i as f64 / ns as f64, e));
    }
    for i in 0..nw {
        pts.push((n, e - (e - w) * i as f64 / nw as f64));
    }
    for i in 0..ns {
        pts.push((n - (n - s) * i as f64 / ns as f64, w));
    }
    poly(&pts)
}

/// Land where longitude < 0, ocean elsewhere.
pub struct WestIsLand;

impl Terrain for WestIsLand {
    fn is_land(&self, p: LatLon) -> Option<bool> {
        Some(p.lon < 0.0)
    }
}
```
(`Terrain` is defined in Task 4. Until then this helper file does not compile; Task 3's test does not `mod common;` — see Step 2. Task 4 onward includes it.)

- [ ] **Step 2: Failing tests**

`crates/wb-constraints/tests/kinds.rs`:
```rust
use wb_constraints::{CONSTRAINT_KINDS, float, is_constraint_kind, polygon, register_kinds, shape, text};
use wb_editlog::{ActorId, AuthorId, EditError, EditLog, FixedClock, Value};
use wb_grid::LatLon;
use wb_world::Geometry;

fn log() -> EditLog {
    let mut log = EditLog::new(ActorId([1; 16]), AuthorId([9; 16]), Box::new(FixedClock(0)));
    register_kinds(&mut log);
    log
}

fn sq() -> Value {
    Value::Geometry(Geometry::Polygon(vec![
        LatLon::from_degrees(0.0, 0.0),
        LatLon::from_degrees(0.0, 1.0),
        LatLon::from_degrees(1.0, 1.0),
    ]))
}

#[test]
fn catalog() {
    assert_eq!(CONSTRAINT_KINDS.len(), 11);
    assert!(is_constraint_kind("lore.area"));
    assert!(!is_constraint_kind("import.base_map"));
}

#[test]
fn valid_entities_commit_and_accessors_read_them() {
    let mut log = log();
    let mut tx = log.transact("forest");
    let f = tx.create("feature.forest", [("area", sq()), ("biome", Value::from("boreal")), ("tolerance", Value::Float(0.2))]);
    tx.commit().unwrap();
    let v = log.state().entity(f).unwrap();
    assert_eq!(polygon(&v, "area").unwrap().len(), 3);
    assert_eq!(text(&v, "biome"), Some("boreal"));
    assert_eq!(float(&v, "tolerance"), Some(0.2));
    let (pts, closed) = shape(&v).unwrap();
    assert_eq!((pts.len(), closed), (3, true));
}

fn rejected(kind: &str, fields: Vec<(&str, Value)>) {
    let mut log = log();
    let mut tx = log.transact("bad");
    tx.create(kind, fields);
    assert!(matches!(tx.commit().unwrap_err(), EditError::ValidationFailed { .. }), "{kind}");
}

#[test]
fn validators_reject_bad_shapes_and_enums() {
    rejected("feature.forest", vec![]);
    rejected("feature.forest", vec![("area", sq()), ("biome", Value::from("jungle"))]);
    rejected("feature.river", vec![("path", Value::Geometry(Geometry::LineString(vec![LatLon::from_degrees(0.0, 0.0)])))]);
    rejected("civ.settlement", vec![("at", Value::LatLon(LatLon::from_degrees(0.0, 0.0)))]);
    rejected("rule.region", vec![("area", sq()), ("rule", Value::from("no_gravity"))]);
    rejected("lore.area", vec![("value", Value::Float(10.0)), ("unit", Value::from("km2"))]);
    rejected("lore.area", vec![
        ("subject", Value::Entity(wb_editlog::EntityId::PLANET)),
        ("value", Value::Float(-1.0)),
        ("unit", Value::from("km2")),
    ]);
    rejected("feature.desert", vec![("area", sq()), ("tolerance", Value::Float(1.5))]);
    rejected("feature.desert", vec![("area", sq()), ("strength", Value::from("firm"))]);
    rejected("feature.hills", vec![("peak_m", Value::Float(300.0))]);
}
```
Run: `cargo test -p wb-constraints --test kinds` — expected FAIL (unresolved imports).

- [ ] **Step 3: Implement**

`crates/wb-constraints/src/kinds.rs`:
```rust
use wb_editlog::{EditLog, EntityId, EntityKind, EntityView, Value};
use wb_grid::LatLon;
use wb_world::Geometry;

pub const CONSTRAINT_KINDS: &[&str] = &[
    "feature.mountain_range",
    "feature.hills",
    "feature.forest",
    "feature.desert",
    "feature.lake",
    "feature.glacier",
    "feature.river",
    "civ.settlement",
    "rule.region",
    "lore.area",
    "lore.relative_position",
];

pub fn is_constraint_kind(kind: &str) -> bool {
    CONSTRAINT_KINDS.contains(&kind)
}

pub fn polygon<'a>(e: &EntityView<'a>, field: &str) -> Option<&'a [LatLon]> {
    match e.get(field) {
        Some(Value::Geometry(Geometry::Polygon(r))) => Some(r.as_slice()),
        _ => None,
    }
}

pub fn line<'a>(e: &EntityView<'a>, field: &str) -> Option<&'a [LatLon]> {
    match e.get(field) {
        Some(Value::Geometry(Geometry::LineString(l))) => Some(l.as_slice()),
        _ => None,
    }
}

pub fn point(e: &EntityView<'_>, field: &str) -> Option<LatLon> {
    match e.get(field) {
        Some(Value::LatLon(p)) => Some(*p),
        _ => None,
    }
}

pub fn text<'a>(e: &EntityView<'a>, field: &str) -> Option<&'a str> {
    match e.get(field) {
        Some(Value::Text(s)) => Some(s.as_str()),
        _ => None,
    }
}

pub fn float(e: &EntityView<'_>, field: &str) -> Option<f64> {
    match e.get(field) {
        Some(Value::Float(x)) => Some(*x),
        _ => None,
    }
}

pub fn entity(e: &EntityView<'_>, field: &str) -> Option<EntityId> {
    match e.get(field) {
        Some(Value::Entity(id)) => Some(*id),
        _ => None,
    }
}

/// The entity's geometry as points: `area` (closed), else `spine`/`path`, else `at`.
pub fn shape(e: &EntityView<'_>) -> Option<(Vec<LatLon>, bool)> {
    if let Some(r) = polygon(e, "area") {
        return Some((r.to_vec(), true));
    }
    if let Some(l) = line(e, "spine").or_else(|| line(e, "path")) {
        return Some((l.to_vec(), false));
    }
    point(e, "at").map(|p| (vec![p], false))
}

fn one_of(e: &EntityView<'_>, field: &str, allowed: &[&str], required: bool) -> Result<(), String> {
    match e.get(field) {
        None if !required => Ok(()),
        Some(Value::Text(s)) if allowed.contains(&s.as_str()) => Ok(()),
        _ => Err(format!("{field} must be one of {allowed:?}")),
    }
}

fn need<T>(value: Option<T>, what: &str) -> Result<(), String> {
    value.map(|_| ()).ok_or_else(|| format!("{what} is required"))
}

fn optional_float(e: &EntityView<'_>, field: &str) -> Result<(), String> {
    match e.get(field) {
        None | Some(Value::Float(_)) => Ok(()),
        _ => Err(format!("{field} must be a number")),
    }
}

fn common(e: &EntityView<'_>) -> Result<(), String> {
    one_of(e, "strength", &["target", "hard"], false)?;
    match e.get("tolerance") {
        None => {}
        Some(Value::Float(t)) if (-1.0..=1.0).contains(t) => {}
        _ => return Err("tolerance must be a number in [-1, 1]".into()),
    }
    match e.get("keep_codes") {
        None => {}
        Some(Value::List(items)) if items.iter().all(|v| matches!(v, Value::Text(_))) => {}
        _ => return Err("keep_codes must be a list of text".into()),
    }
    for f in ["name", "keep_reason"] {
        match e.get(f) {
            None | Some(Value::Text(_)) => {}
            _ => return Err(format!("{f} must be text")),
        }
    }
    Ok(())
}

fn validate(kind: &str, e: &EntityView<'_>) -> Result<(), String> {
    common(e)?;
    match kind {
        "feature.mountain_range" | "feature.hills" => {
            need(line(e, "spine").or_else(|| polygon(e, "area")), "spine or area")?;
            optional_float(e, "peak_m")
        }
        "feature.forest" => {
            need(polygon(e, "area"), "area")?;
            one_of(e, "biome", &["tropical", "temperate", "boreal"], false)
        }
        "feature.desert" | "feature.lake" => need(polygon(e, "area"), "area"),
        "feature.glacier" => {
            need(polygon(e, "area"), "area")?;
            optional_float(e, "elevation_m")
        }
        "feature.river" => match line(e, "path") {
            Some(p) if p.len() >= 2 => Ok(()),
            _ => Err("path must be a line with at least 2 points".into()),
        },
        "civ.settlement" => {
            need(point(e, "at"), "at")?;
            need(text(e, "name"), "name")
        }
        "rule.region" => {
            need(polygon(e, "area"), "area")?;
            one_of(e, "rule", &["no_rain", "permanent_ice", "perpetual_storm"], true)
        }
        "lore.area" => {
            need(entity(e, "subject"), "subject")?;
            match float(e, "value") {
                Some(v) if v > 0.0 => {}
                _ => return Err("value must be a positive number".into()),
            }
            one_of(e, "unit", &["km2", "mi2"], true)
        }
        "lore.relative_position" => {
            need(entity(e, "subject"), "subject")?;
            need(entity(e, "object"), "object")?;
            one_of(e, "relation", &["north_of", "south_of", "east_of", "west_of"], true)
        }
        _ => Ok(()),
    }
}

/// Registers type/enumeration validators for every catalog kind.
pub fn register_kinds(log: &mut EditLog) {
    for &kind in CONSTRAINT_KINDS {
        let k = EntityKind::new(kind).expect("catalog kinds are valid keys");
        log.register_validator(k, Box::new(move |e: EntityView<'_>| validate(kind, &e)));
    }
}
```

Add to `src/lib.rs`:
```rust
mod kinds;
pub use kinds::{CONSTRAINT_KINDS, entity, float, is_constraint_kind, line, point, polygon, register_kinds, shape, text};
```

- [ ] **Step 4: Run, gate, commit**

Run: `cargo test -p wb-constraints --test kinds` — expected PASS (3 tests).
Run: `cargo fmt --all && scripts/check.sh` — expected green. (`tests/common/mod.rs` is not compiled yet because no test file includes it.)
```bash
git add crates/wb-constraints
git commit -m "feat(constraints): kind catalog with validators and field accessors" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: Checker interface, registry, evaluation, explanation templates

**Files:**
- Create: `crates/wb-constraints/src/checker.rs`, `src/evaluate.rs`, `src/templates.rs`
- Modify: `crates/wb-constraints/src/lib.rs`
- Test: `crates/wb-constraints/tests/evaluate.rs`

**Interfaces:**
- Consumes: Tasks 1–3; `wb_editlog::{State, EntityView, EntityId, Value}`; `wb_world::Planet`.
- Produces:
  - `pub enum KindPattern { Exact(&'static str), Prefix(&'static str) }` with `matches(&self, kind: &str) -> bool`.
  - `pub trait Terrain: Send + Sync { fn is_land(&self, p: LatLon) -> Option<bool>; }`; `pub struct UnknownTerrain;` (always `None`).
  - `pub struct CheckContext<'a> { pub state: &'a State, pub planet: Planet, pub realism: f64, pub terrain: &'a dyn Terrain }` with `CheckContext::new(state, planet, terrain)` (realism from `realism_of(state)`).
  - `pub type ConstraintView<'a> = EntityView<'a>;`
  - `pub trait Checker: Send + Sync { fn id(&self) -> &'static str; fn kinds(&self) -> &[KindPattern]; fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding>; }`
  - `pub struct CheckerRegistry` (`Default`): `new()`, `register(Box<dyn Checker>) -> Result<(), ConstraintError>`, `len()`, `is_empty()`, `ids() -> Vec<&'static str>`, crate-internal `iter()`.
  - `realism_of(&State) -> f64` (planet `realism` Float clamped to [0, 1], else 0.5).
  - `evaluate(ctx: &CheckContext<'_>, registry: &CheckerRegistry) -> Report` — live constraints (catalog kinds) in `EntityId` order; matching checkers in id order; non-issue badness forced to 0; issue badness NaN → 1, clamped to [0, 1]; findings sorted by `(checker, code, postcard(params))`; `tolerance` and `keep_codes` read from the entity.
  - `render(&Finding) -> String`; `render_suggestion(&Suggestion) -> String`. Templates use `{name}` placeholders; `Param::Float` formats with no decimals when |x| ≥ 100, else 2 decimals; `Param::Int` plain; `Param::Text` as-is; `Param::Entity` as `#<lamport>.<n>`. Unknown code → `"<code> k=v …"`.

- [ ] **Step 1: Failing tests**

`crates/wb-constraints/tests/evaluate.rs`:
```rust
mod common;

use common::{add, new_log, poly};
use wb_constraints::{
    CheckContext, Checker, CheckerRegistry, ConstraintError, ConstraintView, Finding, Grade, KindPattern, Param, Status,
    UnknownTerrain, evaluate, realism_of, render,
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
        &[KindPattern::Exact("feature.desert"), KindPattern::Prefix("feature.for")]
    }
    fn check(&self, c: ConstraintView<'_>, _ctx: &CheckContext<'_>) -> Vec<Finding> {
        match c.kind() {
            Some("feature.desert") => vec![Finding::issue("test.probe", "test.too_dry", self.0).with_param("pct", Param::Int(40))],
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
    assert_eq!(r.register(Box::new(Probe(0.1))).unwrap_err(), ConstraintError::DuplicateChecker("test.probe".into()));
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
    assert_eq!(evaluate(&ctx, &reg).verdicts[0].grade, Grade::Implausible, "r = 0: 0.3 < 0.4");

    let mut tx = log.transact("keep");
    tx.set(desert, "keep_codes", Value::List(vec![Value::from("test.too_dry")]));
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
    assert_eq!(render(&f), "Drawn area is 1234568 mi2 but the lore says 600000 mi2 (2.06× off).");
    let unknown = Finding::issue("x", "made.up", 0.1).with_param("n", Param::Int(3));
    assert_eq!(render(&unknown), "made.up n=3");
}
```
Run: `cargo test -p wb-constraints --test evaluate` — expected FAIL (unresolved imports).

- [ ] **Step 2: Implement the checker module**

`crates/wb-constraints/src/checker.rs`:
```rust
use crate::error::ConstraintError;
use crate::evaluate::realism_of;
use crate::model::Finding;
use std::collections::BTreeMap;
use wb_editlog::{EntityView, State};
use wb_grid::LatLon;
use wb_world::Planet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KindPattern {
    Exact(&'static str),
    Prefix(&'static str),
}

impl KindPattern {
    pub fn matches(&self, kind: &str) -> bool {
        match self {
            KindPattern::Exact(k) => kind == *k,
            KindPattern::Prefix(p) => kind.starts_with(p),
        }
    }
}

/// What checkers may ask about the physical world. `None` = not known yet.
pub trait Terrain: Send + Sync {
    fn is_land(&self, p: LatLon) -> Option<bool>;
}

/// Terrain before any map is imported or generated.
pub struct UnknownTerrain;

impl Terrain for UnknownTerrain {
    fn is_land(&self, _p: LatLon) -> Option<bool> {
        None
    }
}

pub struct CheckContext<'a> {
    pub state: &'a State,
    pub planet: Planet,
    pub realism: f64,
    pub terrain: &'a dyn Terrain,
}

impl<'a> CheckContext<'a> {
    pub fn new(state: &'a State, planet: Planet, terrain: &'a dyn Terrain) -> Self {
        CheckContext { state, planet, realism: realism_of(state), terrain }
    }
}

pub type ConstraintView<'a> = EntityView<'a>;

/// A pure, deterministic check over one constraint.
pub trait Checker: Send + Sync {
    fn id(&self) -> &'static str;
    fn kinds(&self) -> &[KindPattern];
    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding>;
}

#[derive(Default)]
pub struct CheckerRegistry {
    checkers: BTreeMap<&'static str, Box<dyn Checker>>,
}

impl CheckerRegistry {
    pub fn new() -> Self {
        CheckerRegistry::default()
    }

    pub fn register(&mut self, checker: Box<dyn Checker>) -> Result<(), ConstraintError> {
        let id = checker.id();
        if self.checkers.contains_key(id) {
            return Err(ConstraintError::DuplicateChecker(id.to_string()));
        }
        self.checkers.insert(id, checker);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.checkers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.checkers.is_empty()
    }

    pub fn ids(&self) -> Vec<&'static str> {
        self.checkers.keys().copied().collect()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &dyn Checker> {
        self.checkers.values().map(|c| c.as_ref())
    }
}
```

- [ ] **Step 3: Implement evaluation**

`crates/wb-constraints/src/evaluate.rs`:
```rust
use crate::checker::{CheckContext, CheckerRegistry};
use crate::grade::{effective_realism, make_verdict};
use crate::kinds::{float, is_constraint_kind};
use crate::model::{Finding, FindingKind, Preset, Report};
use std::collections::BTreeSet;
use wb_editlog::{EntityId, State, Value};

/// The world's realism slider: planet `realism` (clamped to [0, 1]), else Plausible fantasy.
pub fn realism_of(state: &State) -> f64 {
    match state.field(EntityId::PLANET, "realism") {
        Some(Value::Float(r)) => r.clamp(0.0, 1.0),
        _ => Preset::PlausibleFantasy.value(),
    }
}

fn sort_key(f: &Finding) -> (String, String, Vec<u8>) {
    (f.checker.clone(), f.code.0.clone(), postcard::to_allocvec(&f.params).expect("params always serialize"))
}

/// Evaluates every live constraint with every matching checker.
pub fn evaluate(ctx: &CheckContext<'_>, registry: &CheckerRegistry) -> Report {
    let mut verdicts = Vec::new();
    for view in ctx.state.live() {
        let Some(kind) = view.kind() else { continue };
        if !is_constraint_kind(kind) {
            continue;
        }
        let mut findings: Vec<Finding> = registry
            .iter()
            .filter(|c| c.kinds().iter().any(|p| p.matches(kind)))
            .flat_map(|c| c.check(view, ctx))
            .collect();
        for f in &mut findings {
            f.badness = match f.kind {
                FindingKind::Issue if f.badness.is_nan() => 1.0,
                FindingKind::Issue => f.badness.clamp(0.0, 1.0),
                _ => 0.0,
            };
        }
        findings.sort_by_cached_key(sort_key);
        let tolerance = float(&view, "tolerance").unwrap_or(0.0);
        let keep: BTreeSet<String> = match view.get("keep_codes") {
            Some(Value::List(items)) => items
                .iter()
                .filter_map(|v| match v {
                    Value::Text(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            _ => BTreeSet::new(),
        };
        verdicts.push(make_verdict(view.id, kind, findings, &keep, effective_realism(ctx.realism, tolerance)));
    }
    Report::new(verdicts)
}
```

- [ ] **Step 4: Implement templates**

`crates/wb-constraints/src/templates.rs`:
```rust
use crate::model::{Finding, Param, Suggestion};
use std::collections::BTreeMap;

fn template(code: &str) -> Option<&'static str> {
    Some(match code {
        "coast.over_ocean" => "{pct}% of this feature lies over the ocean.",
        "lore.area_mismatch" => "Drawn area is {drawn} {unit} but the lore says {stated} {unit} ({ratio}× off).",
        "lore.area_consistent" => "Drawn area matches the lore ({drawn} {unit}).",
        "lore.set_area_to_drawn" => "set the stated area to {drawn} {unit}",
        "lore.relative_position_violated" => "The lore says this lies {relation} its reference, but it is {degrees}° the wrong way.",
        "lore.relative_position_ok" => "Placement matches the lore ({relation}).",
        "rule.forbidden_feature" => "A {feature_kind} lies inside a {rule} region.",
        "river.inland_mouth" => "This river ends inland without a lake.",
        "river.source_in_ocean" => "This river's source is in the ocean.",
        "climate.lowland_tropical_glacier" => "A lowland glacier at {lat}° latitude is unlikely without high elevation.",
        "climate.tropical_forest_high_latitude" => "Tropical forest at {lat}° latitude is unlikely.",
        "climate.boreal_forest_low_latitude" => "Boreal forest at {lat}° latitude is unlikely.",
        _ => return None,
    })
}

fn fmt_param(p: &Param) -> String {
    match p {
        Param::Int(i) => i.to_string(),
        Param::Float(x) if x.abs() >= 100.0 => format!("{x:.0}"),
        Param::Float(x) => format!("{x:.2}"),
        Param::Text(s) => s.clone(),
        Param::Entity(e) => format!("#{}.{}", e.op.lamport, e.n),
    }
}

fn fill(code: &str, params: &BTreeMap<String, Param>) -> String {
    match template(code) {
        Some(t) => {
            let mut out = t.to_string();
            for (k, v) in params {
                out = out.replace(&format!("{{{k}}}"), &fmt_param(v));
            }
            out
        }
        None => {
            let mut out = code.to_string();
            for (k, v) in params {
                out.push_str(&format!(" {k}={}", fmt_param(v)));
            }
            out
        }
    }
}

/// Human-readable explanation of a finding.
pub fn render(f: &Finding) -> String {
    fill(f.code.as_str(), &f.params)
}

/// Human-readable description of a suggestion.
pub fn render_suggestion(s: &Suggestion) -> String {
    fill(s.code.as_str(), &s.params)
}
```

Add to `src/lib.rs`:
```rust
mod checker;
mod evaluate;
mod templates;
pub use checker::{CheckContext, Checker, CheckerRegistry, ConstraintView, KindPattern, Terrain, UnknownTerrain};
pub use evaluate::{evaluate, realism_of};
pub use templates::{render, render_suggestion};
```

- [ ] **Step 5: Run, gate, commit**

Run: `cargo test -p wb-constraints --test evaluate` — expected PASS (5 tests).
Run: `cargo fmt --all && scripts/check.sh` — expected green.
```bash
git add crates/wb-constraints
git commit -m "feat(constraints): checker registry, evaluation, explanation templates" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Actions — suggestions, keep anyway, realism

**Files:**
- Create: `crates/wb-constraints/src/actions.rs`
- Modify: `crates/wb-constraints/src/lib.rs`
- Test: `crates/wb-constraints/tests/actions.rs`

**Interfaces:**
- Consumes: `EditLog` (`transact`, `state`), `render_suggestion`, `is_constraint_kind`, model types.
- Produces:
  - `apply_suggestion(&mut EditLog, &Suggestion) -> Result<OpId, ConstraintError>` — `UnknownEntity` (no change) if any target is missing; label `"Suggestion: <render_suggestion>"` clipped to 200 bytes.
  - `keep_anyway(&mut EditLog, constraint: EntityId, codes: &[IssueCode], reason: Option<&str>) -> Result<OpId, ConstraintError>` — `UnknownEntity` / `NotAConstraint`; unions into `keep_codes` (sorted, deduplicated `List` of `Text`); sets `keep_reason` if given; label `"Kept anyway: <name or kind>"`.
  - `unkeep(&mut EditLog, constraint, codes: &[IssueCode]) -> Result<OpId, ConstraintError>` — removes codes; when none remain writes `Null` to `keep_codes` and `keep_reason`; nothing to change → `Edit(EmptyTransaction)`.
  - `set_realism(&mut EditLog, r: f64) -> Result<OpId, ConstraintError>` — writes planet `realism = clamp(r, 0, 1)`; label `"Realism: <r:.2>"`.

- [ ] **Step 1: Failing tests**

`crates/wb-constraints/tests/actions.rs`:
```rust
mod common;

use common::{add, new_log, poly};
use std::collections::BTreeMap;
use wb_constraints::{ConstraintError, IssueCode, Param, Preset, Suggestion, apply_suggestion, keep_anyway, realism_of, set_realism, unkeep};
use wb_editlog::{EditError, EntityId, Value};

fn desert(log: &mut wb_editlog::EditLog) -> EntityId {
    add(log, "feature.desert", vec![("area", poly(&[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)])), ("name", Value::from("Sand Sea"))])
}

#[test]
fn suggestions_apply_as_undoable_transactions() {
    let mut log = new_log();
    let d = desert(&mut log);
    let s = Suggestion {
        code: IssueCode::new("lore.set_area_to_drawn"),
        params: [("drawn".to_string(), Param::Float(1234.0)), ("unit".to_string(), Param::Text("km2".into()))].into_iter().collect(),
        writes: vec![(d, "name".to_string(), Value::from("Great Sand Sea"))],
    };
    let op = apply_suggestion(&mut log, &s).unwrap();
    assert_eq!(log.op(op).unwrap().label, "Suggestion: set the stated area to 1234 km2");
    assert_eq!(log.state().field(d, "name"), Some(&Value::from("Great Sand Sea")));
    log.undo().unwrap();
    assert_eq!(log.state().field(d, "name"), Some(&Value::from("Sand Sea")));

    let ghost = EntityId { op: wb_editlog::OpId { lamport: 99, actor: wb_editlog::ActorId([5; 16]) }, n: 0 };
    let bad = Suggestion { code: IssueCode::new("x"), params: BTreeMap::new(), writes: vec![(ghost, "name".into(), Value::from("x"))] };
    let before = log.source_hash();
    assert_eq!(apply_suggestion(&mut log, &bad).unwrap_err(), ConstraintError::UnknownEntity(ghost));
    assert_eq!(log.source_hash(), before);
}

#[test]
fn keep_anyway_and_unkeep() {
    let mut log = new_log();
    let d = desert(&mut log);
    let op = keep_anyway(&mut log, d, &[IssueCode::new("b.two"), IssueCode::new("a.one")], Some("Raised by the gods")).unwrap();
    assert_eq!(log.op(op).unwrap().label, "Kept anyway: Sand Sea");
    assert_eq!(
        log.state().field(d, "keep_codes"),
        Some(&Value::List(vec![Value::from("a.one"), Value::from("b.two")]))
    );
    assert_eq!(log.state().field(d, "keep_reason"), Some(&Value::from("Raised by the gods")));
    keep_anyway(&mut log, d, &[IssueCode::new("a.one")], None).unwrap();
    unkeep(&mut log, d, &[IssueCode::new("a.one")]).unwrap();
    assert_eq!(log.state().field(d, "keep_codes"), Some(&Value::List(vec![Value::from("b.two")])));
    unkeep(&mut log, d, &[IssueCode::new("b.two")]).unwrap();
    assert_eq!(log.state().field(d, "keep_codes"), None);
    assert_eq!(log.state().field(d, "keep_reason"), None);
    assert_eq!(unkeep(&mut log, d, &[IssueCode::new("b.two")]).unwrap_err(), ConstraintError::Edit(EditError::EmptyTransaction));

    let not = add(&mut log, "import.base_map", vec![]);
    assert_eq!(keep_anyway(&mut log, not, &[IssueCode::new("x")], None).unwrap_err(), ConstraintError::NotAConstraint(not));
}

#[test]
fn realism_setting() {
    let mut log = new_log();
    set_realism(&mut log, Preset::HighFantasy.value()).unwrap();
    assert_eq!(realism_of(log.state()), 0.85);
    set_realism(&mut log, 7.0).unwrap();
    assert_eq!(realism_of(log.state()), 1.0);
    assert!(matches!(set_realism(&mut log, f64::NAN).unwrap_err(), ConstraintError::Edit(_)));
    log.undo().unwrap();
    assert_eq!(realism_of(log.state()), 0.85);
}
```
Run: `cargo test -p wb-constraints --test actions` — expected FAIL.

- [ ] **Step 2: Implement**

`crates/wb-constraints/src/actions.rs`:
```rust
use crate::error::ConstraintError;
use crate::kinds::{is_constraint_kind, text};
use crate::model::{IssueCode, Suggestion};
use crate::templates::render_suggestion;
use std::collections::BTreeSet;
use wb_editlog::{EditLog, EntityId, MAX_LABEL_BYTES, OpId, Value};

fn clip(s: String) -> String {
    if s.len() <= MAX_LABEL_BYTES {
        return s;
    }
    let mut end = MAX_LABEL_BYTES;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// Commits a suggestion's writes as one transaction.
pub fn apply_suggestion(log: &mut EditLog, s: &Suggestion) -> Result<OpId, ConstraintError> {
    if let Some((e, _, _)) = s.writes.iter().find(|(e, _, _)| log.state().entity(*e).is_none()) {
        return Err(ConstraintError::UnknownEntity(*e));
    }
    let label = clip(format!("Suggestion: {}", render_suggestion(s)));
    let mut tx = log.transact(&label);
    for (e, field, value) in &s.writes {
        tx.set(*e, field, value.clone());
    }
    Ok(tx.commit()?)
}

fn kept(log: &EditLog, constraint: EntityId) -> Result<(BTreeSet<String>, String), ConstraintError> {
    let view = log.state().entity(constraint).ok_or(ConstraintError::UnknownEntity(constraint))?;
    let kind = view.kind().unwrap_or_default();
    if !is_constraint_kind(kind) {
        return Err(ConstraintError::NotAConstraint(constraint));
    }
    let codes = match view.get("keep_codes") {
        Some(Value::List(items)) => items
            .iter()
            .filter_map(|v| match v {
                Value::Text(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => BTreeSet::new(),
    };
    let display = text(&view, "name").unwrap_or(kind).to_string();
    Ok((codes, display))
}

fn codes_value(codes: &BTreeSet<String>) -> Value {
    Value::List(codes.iter().map(|c| Value::Text(c.clone())).collect())
}

pub fn keep_anyway(
    log: &mut EditLog,
    constraint: EntityId,
    codes: &[IssueCode],
    reason: Option<&str>,
) -> Result<OpId, ConstraintError> {
    let (mut current, display) = kept(log, constraint)?;
    current.extend(codes.iter().map(|c| c.0.clone()));
    let mut tx = log.transact(&clip(format!("Kept anyway: {display}")));
    tx.set(constraint, "keep_codes", codes_value(&current));
    if let Some(r) = reason {
        tx.set(constraint, "keep_reason", r);
    }
    Ok(tx.commit()?)
}

pub fn unkeep(log: &mut EditLog, constraint: EntityId, codes: &[IssueCode]) -> Result<OpId, ConstraintError> {
    let (mut current, display) = kept(log, constraint)?;
    let before = current.len();
    for c in codes {
        current.remove(c.as_str());
    }
    let mut tx = log.transact(&clip(format!("Un-kept: {display}")));
    if current.len() != before {
        if current.is_empty() {
            tx.set(constraint, "keep_codes", Value::Null);
            tx.set(constraint, "keep_reason", Value::Null);
        } else {
            tx.set(constraint, "keep_codes", codes_value(&current));
        }
    }
    Ok(tx.commit()?)
}

pub fn set_realism(log: &mut EditLog, r: f64) -> Result<OpId, ConstraintError> {
    let value = if r.is_nan() { r } else { r.clamp(0.0, 1.0) };
    let mut tx = log.transact(&format!("Realism: {value:.2}"));
    tx.set(EntityId::PLANET, "realism", value);
    Ok(tx.commit()?)
}
```
(If `keep_reason` is `Null`-written while absent, the Edit Log records a no-op write; that is harmless.)

Add to `src/lib.rs`:
```rust
mod actions;
pub use actions::{apply_suggestion, keep_anyway, set_realism, unkeep};
```

- [ ] **Step 3: Run, gate, commit**

Run: `cargo test -p wb-constraints --test actions` — expected PASS (3 tests).
Run: `cargo fmt --all && scripts/check.sh` — expected green.
```bash
git add crates/wb-constraints
git commit -m "feat(constraints): apply suggestion, keep anyway, unkeep, set realism" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: Terrain checkers — coast and river mouth

**Files:**
- Create: `crates/wb-constraints/src/checks/mod.rs`, `src/checks/coast.rs`, `src/checks/river_mouth.rs`
- Modify: `crates/wb-constraints/src/lib.rs`
- Test: `crates/wb-constraints/tests/checks_terrain.rs`

**Interfaces:**
- Consumes: `Checker`, `KindPattern`, `CheckContext`, `Finding`, `Param`, `shape`, `line`, `polygon`, `samples`, `contains`.
- Produces: `pub struct CoastCheck;` (id `static.coast`), `pub struct RiverMouthCheck;` (id `static.river_mouth`), exported from `wb_constraints::checks`.
  - Coast: kinds `Prefix("feature.")`, `Exact("civ.settlement")`; ignores `feature.lake`, `feature.river`, `feature.glacier`. Samples `shape` (closed if polygon). If any sample's terrain is `None` → no finding. Ocean fraction `f = ocean / total`; if `f > 0` → issue `coast.over_ocean`, badness `f`, param `pct = Int(round(f·100))`.
  - River mouth: kind `Exact("feature.river")`. `source = path[0]`, `mouth = path[last]`. Mouth `Some(true)` land and not inside any live `feature.lake` `area` → `river.inland_mouth`, 0.3. Source `Some(false)` → `river.source_in_ocean`, 0.8. `None` answers skip that sub-check.

- [ ] **Step 1: Failing tests**

`crates/wb-constraints/tests/checks_terrain.rs`:
```rust
mod common;

use common::{WestIsLand, add, line, new_log, poly};
use wb_constraints::checks::{CoastCheck, RiverMouthCheck};
use wb_constraints::{CheckContext, Checker, UnknownTerrain};
use wb_editlog::Value;
use wb_world::Planet;

fn codes(fs: &[wb_constraints::Finding]) -> Vec<(&str, f64)> {
    fs.iter().map(|f| (f.code.as_str(), f.badness)).collect()
}

#[test]
fn coast_measures_ocean_fraction() {
    let mut log = new_log();
    let inland = add(&mut log, "feature.forest", vec![("area", poly(&[(0.0, -20.0), (0.0, -10.0), (10.0, -10.0), (10.0, -20.0)]))]);
    let straddling = add(&mut log, "feature.desert", vec![("area", poly(&[(0.0, -10.0), (0.0, 10.0), (10.0, 10.0), (10.0, -10.0)]))]);
    let at_sea = add(&mut log, "civ.settlement", vec![("at", Value::LatLon(common::d(0.0, 30.0))), ("name", Value::from("Drowned"))]);
    let lake = add(&mut log, "feature.lake", vec![("area", poly(&[(0.0, 10.0), (0.0, 20.0), (5.0, 20.0)]))]);
    let ctx = CheckContext::new(log.state(), Planet::default(), &WestIsLand);
    let s = log.state();
    assert!(CoastCheck.check(s.entity(inland).unwrap(), &ctx).is_empty());
    let f = CoastCheck.check(s.entity(straddling).unwrap(), &ctx);
    assert_eq!(f.len(), 1);
    assert!(f[0].badness > 0.2 && f[0].badness < 0.8, "{}", f[0].badness);
    assert_eq!(codes(&CoastCheck.check(s.entity(at_sea).unwrap(), &ctx)), vec![("coast.over_ocean", 1.0)]);
    assert!(CoastCheck.check(s.entity(lake).unwrap(), &ctx).is_empty(), "lakes are exempt");

    let unknown = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    assert!(CoastCheck.check(s.entity(at_sea).unwrap(), &unknown).is_empty());
}

#[test]
fn river_mouth_and_source() {
    let mut log = new_log();
    let to_sea = add(&mut log, "feature.river", vec![("path", line(&[(0.0, -20.0), (0.0, 5.0)]))]);
    let inland = add(&mut log, "feature.river", vec![("path", line(&[(0.0, -20.0), (0.0, -5.0)]))]);
    let from_sea = add(&mut log, "feature.river", vec![("path", line(&[(0.0, 20.0), (0.0, -5.0)]))]);
    add(&mut log, "feature.lake", vec![("area", poly(&[(-1.0, -6.0), (-1.0, -4.0), (1.0, -4.0), (1.0, -6.0)]))]);
    let into_nothing = add(&mut log, "feature.river", vec![("path", line(&[(10.0, -20.0), (10.0, -5.0)]))]);
    let ctx = CheckContext::new(log.state(), Planet::default(), &WestIsLand);
    let s = log.state();
    assert!(RiverMouthCheck.check(s.entity(to_sea).unwrap(), &ctx).is_empty());
    assert!(RiverMouthCheck.check(s.entity(inland).unwrap(), &ctx).is_empty(), "ends in the lake");
    assert_eq!(codes(&RiverMouthCheck.check(s.entity(from_sea).unwrap(), &ctx)), vec![("river.source_in_ocean", 0.8)]);
    assert_eq!(codes(&RiverMouthCheck.check(s.entity(into_nothing).unwrap(), &ctx)), vec![("river.inland_mouth", 0.3)]);
}
```
Run: `cargo test -p wb-constraints --test checks_terrain` — expected FAIL.

- [ ] **Step 2: Implement**

`crates/wb-constraints/src/checks/mod.rs`:
```rust
//! Starter checkers that need no simulation.

mod coast;
mod river_mouth;

pub use coast::CoastCheck;
pub use river_mouth::RiverMouthCheck;
```

`crates/wb-constraints/src/checks/coast.rs`:
```rust
use crate::checker::{CheckContext, Checker, ConstraintView, KindPattern};
use crate::geo::samples;
use crate::kinds::shape;
use crate::model::{Finding, Param};

/// Land features and settlements should not sit over the ocean.
pub struct CoastCheck;

const EXEMPT: &[&str] = &["feature.lake", "feature.river", "feature.glacier"];

impl Checker for CoastCheck {
    fn id(&self) -> &'static str {
        "static.coast"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Prefix("feature."), KindPattern::Exact("civ.settlement")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        if c.kind().is_some_and(|k| EXEMPT.contains(&k)) {
            return Vec::new();
        }
        let Some((points, closed)) = shape(&c) else { return Vec::new() };
        let pts = samples(&points, closed);
        let mut ocean = 0usize;
        for p in &pts {
            match ctx.terrain.is_land(*p) {
                None => return Vec::new(),
                Some(false) => ocean += 1,
                Some(true) => {}
            }
        }
        if ocean == 0 || pts.is_empty() {
            return Vec::new();
        }
        let f = ocean as f64 / pts.len() as f64;
        vec![Finding::issue(self.id(), "coast.over_ocean", f).with_param("pct", Param::Int((f * 100.0).round() as i64))]
    }
}
```

`crates/wb-constraints/src/checks/river_mouth.rs`:
```rust
use crate::checker::{CheckContext, Checker, ConstraintView, KindPattern};
use crate::geo::contains;
use crate::kinds::{line, polygon};
use crate::model::Finding;
use wb_grid::LatLon;

/// Rivers should reach the sea or a lake, and start on land.
pub struct RiverMouthCheck;

fn in_a_lake(ctx: &CheckContext<'_>, p: LatLon) -> bool {
    ctx.state
        .live()
        .filter(|e| e.kind() == Some("feature.lake"))
        .filter_map(|e| polygon(&e, "area"))
        .any(|ring| contains(ring, p))
}

impl Checker for RiverMouthCheck {
    fn id(&self) -> &'static str {
        "static.river_mouth"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("feature.river")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(path) = line(&c, "path") else { return Vec::new() };
        let (Some(source), Some(mouth)) = (path.first().copied(), path.last().copied()) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if ctx.terrain.is_land(mouth) == Some(true) && !in_a_lake(ctx, mouth) {
            out.push(Finding::issue(self.id(), "river.inland_mouth", 0.3));
        }
        if ctx.terrain.is_land(source) == Some(false) {
            out.push(Finding::issue(self.id(), "river.source_in_ocean", 0.8));
        }
        out
    }
}
```

Add to `src/lib.rs`:
```rust
pub mod checks;
```

- [ ] **Step 3: Run, gate, commit**

Run: `cargo test -p wb-constraints --test checks_terrain` — expected PASS (2 tests).
Run: `cargo fmt --all && scripts/check.sh` — expected green.
```bash
git add crates/wb-constraints
git commit -m "feat(constraints): coast and river-mouth checkers" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 7: Lore checkers — area and relative position

**Files:**
- Create: `crates/wb-constraints/src/checks/lore_area.rs`, `src/checks/relative_position.rs`
- Modify: `crates/wb-constraints/src/checks/mod.rs`
- Test: `crates/wb-constraints/tests/checks_lore.rs`

**Interfaces:**
- Consumes: `entity`, `float`, `text`, `polygon`, `shape`, `centroid`, `wb_world::polygon_area_m2`, `wb_editlog::wrap_lon`.
- Produces: `LoreAreaCheck` (id `static.lore_area`), `RelativePositionCheck` (id `static.relative_position`).
  - Lore area: subject must be live with a Polygon `area`; unit `km2` (factor 1) or `mi2` (factor 2.589988110336); `drawn = polygon_area_m2 / 1e6 / factor` (in the stated unit); `ratio = max/min`. `ratio ≤ 1.15` → support `lore.area_consistent` (params `drawn`, `unit`). Else issue `lore.area_mismatch`, badness `min(1, (ratio − 1.15) / 1.85)`, params `drawn`, `stated`, `unit`, `ratio`, plus a suggestion (code `lore.set_area_to_drawn`, params `drawn`, `unit`, write `(constraint, "value", Float(drawn))`).
  - Relative position: subject and object live with a `shape`; centroids. `north_of`: violation `d = obj.lat − subj.lat` (degrees) when `subj.lat ≤ obj.lat`; `south_of` mirrored; `east_of`: `dlon = wrap_lon(subj.lon − obj.lon)`, violated when `dlon ≤ 0`, `d = −dlon` in degrees; `west_of` mirrored. Violated → issue `lore.relative_position_violated`, badness `min(1, 0.2 + d/20)`, params `relation`, `degrees`, related `[object]`; satisfied → support `lore.relative_position_ok` with `relation`.

- [ ] **Step 1: Failing tests**

`crates/wb-constraints/tests/checks_lore.rs`:
```rust
mod common;

use common::{add, band, new_log};
use wb_constraints::checks::{LoreAreaCheck, RelativePositionCheck};
use wb_constraints::{CheckContext, Checker, FindingKind, Param, UnknownTerrain};
use wb_editlog::Value;
use wb_world::Planet;

#[test]
fn lore_area_compares_drawn_and_stated() {
    let mut log = new_log();
    // ≈ 223 000 km² per degree of longitude between 15°N and 35°N.
    let sea = add(&mut log, "feature.desert", vec![("area", band(15.0, 35.0, 30.0, 47.0))]);
    let wrong = add(&mut log, "lore.area", vec![("subject", Value::Entity(sea)), ("value", Value::Float(600_000.0)), ("unit", Value::from("mi2"))]);
    let right = add(&mut log, "lore.area", vec![("subject", Value::Entity(sea)), ("value", Value::Float(3_790_000.0)), ("unit", Value::from("km2"))]);
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let s = log.state();

    let f = LoreAreaCheck.check(s.entity(wrong).unwrap(), &ctx);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].code.as_str(), "lore.area_mismatch");
    let Param::Float(ratio) = f[0].params["ratio"] else { panic!() };
    assert!(ratio > 2.3 && ratio < 2.6, "ratio {ratio}");
    assert!((f[0].badness - (ratio - 1.15) / 1.85).abs() < 1e-12);
    let sug = &f[0].suggestions[0];
    assert_eq!(sug.code.as_str(), "lore.set_area_to_drawn");
    assert_eq!(sug.writes[0].0, wrong);
    assert_eq!(sug.writes[0].1, "value");

    let ok = LoreAreaCheck.check(s.entity(right).unwrap(), &ctx);
    assert_eq!(ok.len(), 1);
    assert_eq!(ok[0].kind, FindingKind::Support);
}

#[test]
fn relative_position() {
    let mut log = new_log();
    // Sea centroid ≈ (25°N, 38.5°E); mountains centroid ≈ (51°N, 27°E): ~26° north, ~11° west.
    let sea = add(&mut log, "feature.desert", vec![("area", band(15.0, 35.0, 30.0, 47.0))]);
    let north = add(&mut log, "feature.mountain_range", vec![("spine", common::line(&[(50.0, 20.0), (52.0, 35.0)]))]);
    let rel = |log: &mut wb_editlog::EditLog, s, o, r: &str| {
        add(log, "lore.relative_position", vec![("subject", Value::Entity(s)), ("object", Value::Entity(o)), ("relation", Value::from(r))])
    };
    let ok = rel(&mut log, north, sea, "north_of");
    let bad = rel(&mut log, sea, north, "north_of");
    let west = rel(&mut log, north, sea, "west_of");
    let east = rel(&mut log, north, sea, "east_of");
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let s = log.state();

    assert_eq!(RelativePositionCheck.check(s.entity(ok).unwrap(), &ctx)[0].kind, FindingKind::Support);
    let f = RelativePositionCheck.check(s.entity(bad).unwrap(), &ctx);
    assert_eq!(f[0].code.as_str(), "lore.relative_position_violated");
    assert_eq!(f[0].badness, 1.0, "≈ 26° wrong way → 0.2 + 26/20 capped at 1");
    assert_eq!(f[0].related, vec![north]);
    assert_eq!(RelativePositionCheck.check(s.entity(west).unwrap(), &ctx)[0].kind, FindingKind::Support);
    let e = RelativePositionCheck.check(s.entity(east).unwrap(), &ctx);
    assert_eq!(e[0].code.as_str(), "lore.relative_position_violated");
    assert!(e[0].badness > 0.6 && e[0].badness < 0.9, "≈ 11° west → ≈ 0.75, got {}", e[0].badness);
}
```

If an assertion here fails, compute the actual centroids before changing anything and report them; the margins above are deliberately wide.

Run: `cargo test -p wb-constraints --test checks_lore` — expected FAIL.

- [ ] **Step 2: Implement**

`crates/wb-constraints/src/checks/lore_area.rs`:
```rust
use crate::checker::{CheckContext, Checker, ConstraintView, KindPattern};
use crate::kinds::{entity, float, polygon, text};
use crate::model::{Finding, IssueCode, Param, Suggestion};
use wb_editlog::Value;
use wb_world::polygon_area_m2;

pub const MI2_IN_KM2: f64 = 2.589988110336;

/// A lore-stated area should match the drawn area.
pub struct LoreAreaCheck;

impl Checker for LoreAreaCheck {
    fn id(&self) -> &'static str {
        "static.lore_area"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("lore.area")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let (Some(subject), Some(stated), Some(unit)) = (entity(&c, "subject"), float(&c, "value"), text(&c, "unit")) else {
            return Vec::new();
        };
        let factor = match unit {
            "km2" => 1.0,
            "mi2" => MI2_IN_KM2,
            _ => return Vec::new(),
        };
        let Some(view) = ctx.state.entity(subject).filter(|v| !v.is_deleted()) else { return Vec::new() };
        let Some(ring) = polygon(&view, "area") else { return Vec::new() };
        let drawn = polygon_area_m2(&ctx.planet, ring) / 1e6 / factor;
        if drawn <= 0.0 || stated <= 0.0 {
            return Vec::new();
        }
        let ratio = drawn.max(stated) / drawn.min(stated);
        let unit_p = Param::Text(unit.to_string());
        if ratio <= 1.15 {
            return vec![Finding::support(self.id(), "lore.area_consistent")
                .with_param("drawn", Param::Float(drawn))
                .with_param("unit", unit_p)];
        }
        let suggestion = Suggestion {
            code: IssueCode::new("lore.set_area_to_drawn"),
            params: [("drawn".to_string(), Param::Float(drawn)), ("unit".to_string(), unit_p.clone())].into_iter().collect(),
            writes: vec![(c.id, "value".to_string(), Value::Float(drawn))],
        };
        vec![Finding::issue(self.id(), "lore.area_mismatch", ((ratio - 1.15) / 1.85).min(1.0))
            .with_param("drawn", Param::Float(drawn))
            .with_param("stated", Param::Float(stated))
            .with_param("unit", unit_p)
            .with_param("ratio", Param::Float(ratio))
            .with_related(subject)
            .with_suggestion(suggestion)]
    }
}
```

`crates/wb-constraints/src/checks/relative_position.rs`:
```rust
use crate::checker::{CheckContext, Checker, ConstraintView, KindPattern};
use crate::geo::centroid;
use crate::kinds::{entity, shape, text};
use crate::model::{Finding, Param};
use wb_editlog::{EntityId, wrap_lon};
use wb_grid::LatLon;

/// "X lies north/south/east/west of Y" should match the drawing.
pub struct RelativePositionCheck;

fn center(ctx: &CheckContext<'_>, id: EntityId) -> Option<LatLon> {
    let view = ctx.state.entity(id).filter(|v| !v.is_deleted())?;
    let (points, _) = shape(&view)?;
    centroid(&points)
}

impl Checker for RelativePositionCheck {
    fn id(&self) -> &'static str {
        "static.relative_position"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("lore.relative_position")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let (Some(s), Some(o), Some(relation)) = (entity(&c, "subject"), entity(&c, "object"), text(&c, "relation")) else {
            return Vec::new();
        };
        let (Some(sp), Some(op)) = (center(ctx, s), center(ctx, o)) else { return Vec::new() };
        let dlon = wrap_lon(sp.lon - op.lon);
        // Positive `margin` = satisfied by that many radians; negative = violated.
        let margin = match relation {
            "north_of" => sp.lat - op.lat,
            "south_of" => op.lat - sp.lat,
            "east_of" => dlon,
            "west_of" => -dlon,
            _ => return Vec::new(),
        };
        let rel = Param::Text(relation.to_string());
        if margin > 0.0 {
            return vec![Finding::support(self.id(), "lore.relative_position_ok").with_param("relation", rel)];
        }
        let d = (-margin).to_degrees();
        vec![Finding::issue(self.id(), "lore.relative_position_violated", (0.2 + d / 20.0).min(1.0))
            .with_param("relation", rel)
            .with_param("degrees", Param::Float(d))
            .with_related(o)]
    }
}
```

Add to `src/checks/mod.rs`:
```rust
mod lore_area;
mod relative_position;
pub use lore_area::{LoreAreaCheck, MI2_IN_KM2};
pub use relative_position::RelativePositionCheck;
```

- [ ] **Step 3: Run, gate, commit**

Run: `cargo test -p wb-constraints --test checks_lore` — expected PASS (2 tests).
Run: `cargo fmt --all && scripts/check.sh` — expected green.
```bash
git add crates/wb-constraints
git commit -m "feat(constraints): lore area and relative-position checkers" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 8: Rule and latitude checkers; starter registry

**Files:**
- Create: `crates/wb-constraints/src/checks/region_rules.rs`, `src/checks/latitude.rs`
- Modify: `crates/wb-constraints/src/checks/mod.rs`
- Test: `crates/wb-constraints/tests/checks_rules.rs`

**Interfaces:**
- Consumes: `polygon`, `shape`, `text`, `float`, `centroid`, `contains`, registry.
- Produces: `RegionRulesCheck` (id `static.region_rules`), `LatitudeSanityCheck` (id `static.latitude_sanity`), `CheckerRegistry::with_starter_checks() -> CheckerRegistry` (all six, ids sorted: `static.coast`, `static.latitude_sanity`, `static.lore_area`, `static.region_rules`, `static.relative_position`, `static.river_mouth`).
  - Region rules: forbidden kinds — `no_rain` → `feature.lake`, `feature.river`, `feature.forest`; `permanent_ice` → `feature.forest`, `feature.desert`, `civ.settlement`; `perpetual_storm` → none. For each live other entity of a forbidden kind whose `shape` centroid is inside the rule `area` → issue `rule.forbidden_feature`, 0.6, params `feature_kind`, `rule`, related `[entity]`.
  - Latitude: glacier: `elevation_m` (absent 0) `< 500` and |centroid lat| ≤ 25° → `climate.lowland_tropical_glacier` 0.6. Forest `biome = tropical` and |lat| > 35° → `climate.tropical_forest_high_latitude` 0.5; `boreal` and |lat| < 30° → `climate.boreal_forest_low_latitude` 0.4. Param `lat` = Float(latitude in degrees, signed).

- [ ] **Step 1: Failing tests**

`crates/wb-constraints/tests/checks_rules.rs`:
```rust
mod common;

use common::{add, new_log, poly};
use wb_constraints::checks::{LatitudeSanityCheck, RegionRulesCheck};
use wb_constraints::{CheckContext, Checker, CheckerRegistry, UnknownTerrain};
use wb_editlog::Value;
use wb_world::Planet;

fn box_at(lat: f64, lon: f64) -> Value {
    poly(&[(lat - 1.0, lon - 1.0), (lat - 1.0, lon + 1.0), (lat + 1.0, lon + 1.0), (lat + 1.0, lon - 1.0)])
}

#[test]
fn region_rules_flag_forbidden_features() {
    let mut log = new_log();
    let dry = add(&mut log, "rule.region", vec![("area", poly(&[(-10.0, -10.0), (-10.0, 10.0), (10.0, 10.0), (10.0, -10.0)])), ("rule", Value::from("no_rain"))]);
    let lake = add(&mut log, "feature.lake", vec![("area", box_at(0.0, 0.0))]);
    add(&mut log, "feature.desert", vec![("area", box_at(0.0, 3.0))]);
    add(&mut log, "feature.forest", vec![("area", box_at(40.0, 40.0))]);
    let storm = add(&mut log, "rule.region", vec![("area", box_at(0.0, 0.0)), ("rule", Value::from("perpetual_storm"))]);
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let f = RegionRulesCheck.check(log.state().entity(dry).unwrap(), &ctx);
    assert_eq!(f.len(), 1);
    assert_eq!((f[0].code.as_str(), f[0].badness), ("rule.forbidden_feature", 0.6));
    assert_eq!(f[0].related, vec![lake]);
    assert!(RegionRulesCheck.check(log.state().entity(storm).unwrap(), &ctx).is_empty());
}

#[test]
fn latitude_sanity() {
    let mut log = new_log();
    let tropical_glacier = add(&mut log, "feature.glacier", vec![("area", box_at(5.0, 0.0))]);
    let high_glacier = add(&mut log, "feature.glacier", vec![("area", box_at(5.0, 0.0)), ("elevation_m", Value::Float(5200.0))]);
    let tropical_north = add(&mut log, "feature.forest", vec![("area", box_at(50.0, 0.0)), ("biome", Value::from("tropical"))]);
    let boreal_south = add(&mut log, "feature.forest", vec![("area", box_at(-10.0, 0.0)), ("biome", Value::from("boreal"))]);
    let fine = add(&mut log, "feature.forest", vec![("area", box_at(60.0, 0.0)), ("biome", Value::from("boreal"))]);
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let s = log.state();
    let code = |e| LatitudeSanityCheck.check(s.entity(e).unwrap(), &ctx).first().map(|f| (f.code.0.clone(), f.badness));
    assert_eq!(code(tropical_glacier), Some(("climate.lowland_tropical_glacier".into(), 0.6)));
    assert_eq!(code(high_glacier), None);
    assert_eq!(code(tropical_north), Some(("climate.tropical_forest_high_latitude".into(), 0.5)));
    assert_eq!(code(boreal_south), Some(("climate.boreal_forest_low_latitude".into(), 0.4)));
    assert_eq!(code(fine), None);
}

#[test]
fn starter_registry() {
    let r = CheckerRegistry::with_starter_checks();
    assert_eq!(
        r.ids(),
        vec!["static.coast", "static.latitude_sanity", "static.lore_area", "static.region_rules", "static.relative_position", "static.river_mouth"]
    );
}
```
Run: `cargo test -p wb-constraints --test checks_rules` — expected FAIL.

- [ ] **Step 2: Implement**

`crates/wb-constraints/src/checks/region_rules.rs`:
```rust
use crate::checker::{CheckContext, Checker, ConstraintView, KindPattern};
use crate::geo::{centroid, contains};
use crate::kinds::{polygon, shape, text};
use crate::model::{Finding, Param};

/// Features a region rule forbids should not lie inside it.
pub struct RegionRulesCheck;

fn forbidden(rule: &str) -> &'static [&'static str] {
    match rule {
        "no_rain" => &["feature.lake", "feature.river", "feature.forest"],
        "permanent_ice" => &["feature.forest", "feature.desert", "civ.settlement"],
        _ => &[],
    }
}

impl Checker for RegionRulesCheck {
    fn id(&self) -> &'static str {
        "static.region_rules"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("rule.region")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let (Some(ring), Some(rule)) = (polygon(&c, "area"), text(&c, "rule")) else { return Vec::new() };
        let banned = forbidden(rule);
        let mut out = Vec::new();
        for other in ctx.state.live() {
            let Some(kind) = other.kind() else { continue };
            if other.id == c.id || !banned.contains(&kind) {
                continue;
            }
            let Some(center) = shape(&other).and_then(|(pts, _)| centroid(&pts)) else { continue };
            if contains(ring, center) {
                out.push(
                    Finding::issue(self.id(), "rule.forbidden_feature", 0.6)
                        .with_param("feature_kind", Param::Text(kind.to_string()))
                        .with_param("rule", Param::Text(rule.to_string()))
                        .with_related(other.id),
                );
            }
        }
        out
    }
}
```

`crates/wb-constraints/src/checks/latitude.rs`:
```rust
use crate::checker::{CheckContext, Checker, ConstraintView, KindPattern};
use crate::geo::centroid;
use crate::kinds::{float, shape, text};
use crate::model::{Finding, Param};

/// Gentle latitude sanity checks; superseded by the climate stage later.
pub struct LatitudeSanityCheck;

impl Checker for LatitudeSanityCheck {
    fn id(&self) -> &'static str {
        "static.latitude_sanity"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("feature.glacier"), KindPattern::Exact("feature.forest")]
    }

    fn check(&self, c: ConstraintView<'_>, _ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(center) = shape(&c).and_then(|(pts, _)| centroid(&pts)) else { return Vec::new() };
        let lat = center.lat.to_degrees();
        let issue = |code: &str, badness: f64| vec![Finding::issue(self.id(), code, badness).with_param("lat", Param::Float(lat))];
        match (c.kind(), text(&c, "biome")) {
            (Some("feature.glacier"), _) if float(&c, "elevation_m").unwrap_or(0.0) < 500.0 && lat.abs() <= 25.0 => {
                issue("climate.lowland_tropical_glacier", 0.6)
            }
            (Some("feature.forest"), Some("tropical")) if lat.abs() > 35.0 => issue("climate.tropical_forest_high_latitude", 0.5),
            (Some("feature.forest"), Some("boreal")) if lat.abs() < 30.0 => issue("climate.boreal_forest_low_latitude", 0.4),
            _ => Vec::new(),
        }
    }
}
```

Update `src/checks/mod.rs` to:
```rust
//! Starter checkers that need no simulation.

mod coast;
mod latitude;
mod lore_area;
mod region_rules;
mod relative_position;
mod river_mouth;

pub use coast::CoastCheck;
pub use latitude::LatitudeSanityCheck;
pub use lore_area::{LoreAreaCheck, MI2_IN_KM2};
pub use region_rules::RegionRulesCheck;
pub use relative_position::RelativePositionCheck;
pub use river_mouth::RiverMouthCheck;

use crate::checker::CheckerRegistry;

impl CheckerRegistry {
    /// The six starter checkers (no simulation needed).
    pub fn with_starter_checks() -> CheckerRegistry {
        let mut r = CheckerRegistry::new();
        for c in [
            Box::new(CoastCheck) as Box<dyn crate::checker::Checker>,
            Box::new(LatitudeSanityCheck),
            Box::new(LoreAreaCheck),
            Box::new(RegionRulesCheck),
            Box::new(RelativePositionCheck),
            Box::new(RiverMouthCheck),
        ] {
            r.register(c).expect("starter checker ids are unique");
        }
        r
    }
}
```

- [ ] **Step 3: Run, gate, commit**

Run: `cargo test -p wb-constraints --test checks_rules` — expected PASS (3 tests).
Run: `cargo fmt --all && scripts/check.sh` — expected green.
```bash
git add crates/wb-constraints
git commit -m "feat(constraints): region-rule and latitude checkers; starter registry" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 9: Aethoria end-to-end, golden, performance guard

**Files:**
- Test: `crates/wb-constraints/tests/aethoria.rs`, `tests/golden.rs`, `tests/perf.rs`
- Modify: `scripts/check.sh`

**Interfaces:**
- Consumes: the full public API.
- Produces: Aethoria acceptance test; cross-target `GOLDEN_REPORT` constant; release-only 5 000-constraint budget (< 200 ms native, < 600 ms wasm32-wasip1) wired into `scripts/check.sh`.

- [ ] **Step 1: Aethoria end-to-end test**

`crates/wb-constraints/tests/aethoria.rs`:
```rust
mod common;

use common::{add, band, line, new_log};
use wb_constraints::{
    CheckContext, CheckerRegistry, Grade, IssueCode, Preset, Status, UnknownTerrain, apply_suggestion, evaluate, keep_anyway,
    set_realism,
};
use wb_editlog::{EditLog, EntityId, Value};
use wb_world::Planet;

fn verdict_grade(log: &EditLog, id: EntityId) -> (Grade, Status) {
    let reg = CheckerRegistry::with_starter_checks();
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let r = evaluate(&ctx, &reg);
    let v = r.verdicts.iter().find(|v| v.constraint == id).expect("verdict present");
    (v.grade, v.status)
}

#[test]
fn great_sand_sea_and_thulean_mountains() {
    let mut log = new_log();
    let sea = add(&mut log, "feature.desert", vec![("area", band(15.0, 35.0, 30.0, 47.0)), ("name", Value::from("Great Sand Sea"))]);
    let thulean = add(&mut log, "feature.mountain_range", vec![("spine", line(&[(38.0, 30.0), (40.0, 47.0)])), ("name", Value::from("Thulean Mountains"))]);
    let area = add(&mut log, "lore.area", vec![("subject", Value::Entity(sea)), ("value", Value::Float(600_000.0)), ("unit", Value::from("mi2"))]);
    let north = add(&mut log, "lore.relative_position", vec![("subject", Value::Entity(thulean)), ("object", Value::Entity(sea)), ("relation", Value::from("north_of"))]);

    set_realism(&mut log, Preset::EarthStrict.value()).unwrap();
    assert_eq!(verdict_grade(&log, area).0, Grade::Implausible);
    set_realism(&mut log, Preset::PlausibleFantasy.value()).unwrap();
    assert_eq!(verdict_grade(&log, area).0, Grade::Stretch);
    assert_eq!(verdict_grade(&log, north).0, Grade::Plausible);

    // Apply the suggestion: lore now matches the map.
    let reg = CheckerRegistry::with_starter_checks();
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let report = evaluate(&ctx, &reg);
    let s = report.verdicts.iter().find(|v| v.constraint == area).unwrap().issues[0].suggestions[0].clone();
    apply_suggestion(&mut log, &s).unwrap();
    assert_eq!(verdict_grade(&log, area).0, Grade::Plausible);
    log.undo().unwrap();
    assert_eq!(verdict_grade(&log, area).0, Grade::Stretch);

    // Keep it anyway instead.
    keep_anyway(&mut log, area, &[IssueCode::new("lore.area_mismatch")], Some("Cartographers exaggerate")).unwrap();
    assert_eq!(verdict_grade(&log, area), (Grade::Plausible, Status::Intentional));

    // Swap the mountains south of the sea.
    let mut tx = log.transact("move mountains");
    tx.set(thulean, "spine", line(&[(5.0, 30.0), (7.0, 47.0)]));
    tx.commit().unwrap();
    assert_eq!(verdict_grade(&log, north).0, Grade::Implausible);
}
```

- [ ] **Step 2: Golden test**

`crates/wb-constraints/tests/golden.rs`:
```rust
//! Cross-target determinism gate for verdict reports. Update GOLDEN_REPORT in the same
//! commit as any intentional change to checkers, grading, or encoding.

mod common;

use common::{WestIsLand, add, band, line, new_log, poly};
use wb_constraints::{CheckContext, CheckerRegistry, IssueCode, evaluate, keep_anyway};
use wb_editlog::Value;
use wb_world::{Planet, to_hex};

const GOLDEN_REPORT: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[test]
fn golden_report() {
    let mut log = new_log();
    let sea = add(&mut log, "feature.desert", vec![("area", band(15.0, 35.0, -47.0, -30.0))]);
    let range = add(&mut log, "feature.mountain_range", vec![("spine", line(&[(38.0, -47.0), (40.0, -30.0)]))]);
    add(&mut log, "lore.area", vec![("subject", Value::Entity(sea)), ("value", Value::Float(600_000.0)), ("unit", Value::from("mi2"))]);
    add(&mut log, "lore.relative_position", vec![("subject", Value::Entity(range)), ("object", Value::Entity(sea)), ("relation", Value::from("south_of"))]);
    add(&mut log, "feature.river", vec![("path", line(&[(20.0, 10.0), (20.0, -5.0)]))]);
    add(&mut log, "feature.glacier", vec![("area", poly(&[(1.0, -20.0), (1.0, -18.0), (3.0, -18.0)]))]);
    let forest = add(&mut log, "feature.forest", vec![("area", poly(&[(50.0, -20.0), (50.0, -18.0), (52.0, -18.0)])), ("biome", Value::from("tropical"))]);
    add(&mut log, "rule.region", vec![("area", poly(&[(45.0, -25.0), (45.0, -10.0), (55.0, -10.0), (55.0, -25.0)])), ("rule", Value::from("no_rain"))]);
    add(&mut log, "civ.settlement", vec![("at", Value::LatLon(common::d(10.0, 20.0))), ("name", Value::from("Kaldros"))]);
    keep_anyway(&mut log, forest, &[IssueCode::new("climate.tropical_forest_high_latitude")], None).unwrap();

    let reg = CheckerRegistry::with_starter_checks();
    let ctx = CheckContext::new(log.state(), Planet::default(), &WestIsLand);
    let got = to_hex(&evaluate(&ctx, &reg).hash);
    assert_eq!(got, GOLDEN_REPORT, "report changed; if intentional, set GOLDEN_REPORT = \"{got}\"");
}
```

- [ ] **Step 3: Performance guard**

`crates/wb-constraints/tests/perf.rs`:
```rust
mod common;

use common::{add, new_log, poly};
use wb_constraints::{CheckContext, CheckerRegistry, UnknownTerrain, evaluate};
use wb_editlog::Value;
use wb_world::Planet;

#[test]
#[cfg_attr(debug_assertions, ignore = "release-only performance guard")]
fn evaluating_5000_constraints_is_within_budget() {
    let mut log = new_log();
    for i in 0..2_500 {
        let lat = -60.0 + (i % 120) as f64;
        let lon = -170.0 + (i / 120) as f64 * 15.0;
        let d = add(&mut log, "feature.desert", vec![("area", poly(&[(lat, lon), (lat, lon + 1.0), (lat + 0.5, lon + 1.0)]))]);
        add(&mut log, "lore.area", vec![("subject", Value::Entity(d)), ("value", Value::Float(3_000.0)), ("unit", Value::from("km2"))]);
    }
    let reg = CheckerRegistry::with_starter_checks();
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    let start = std::time::Instant::now();
    let report = evaluate(&ctx, &reg);
    let ms = start.elapsed().as_millis();
    assert_eq!(report.verdicts.len(), 5_000);
    let budget = if cfg!(target_family = "wasm") { 600 } else { 200 };
    assert!(ms <= budget, "evaluated 5000 constraints in {ms} ms (budget {budget} ms)");
}
```

- [ ] **Step 4: Run and capture the golden**

Run: `cargo test -p wb-constraints --test aethoria` — expected PASS. If an assertion fails, recompute the geometry the test relies on (areas, centroids) and report the real values — do not weaken assertions silently.
Run: `cargo test -p wb-constraints --test golden` — expected FAIL printing `set GOLDEN_REPORT = "<64 hex>"`; paste it into `GOLDEN_REPORT` and re-run — expected PASS.
Run: `cargo test -p wb-constraints --test golden --target wasm32-wasip1` and `cargo test --release -p wb-constraints --test golden` — expected PASS (same hash).
Run: `cargo test --release -p wb-constraints --test perf -- --nocapture` and `cargo test --release -p wb-constraints --test perf --target wasm32-wasip1` — expected PASS; record measured times in the report.

- [ ] **Step 5: Gate integration, commit**

Append to `scripts/check.sh` after the existing wb-editlog perf lines:
```bash
cargo test --release -p wb-constraints --test perf
cargo test --release -p wb-constraints --test perf --target wasm32-wasip1
```
Run: `cargo fmt --all && scripts/ci-local.sh` — expected green, including `ci-local: x86-64 tests passed`.
```bash
git add crates/wb-constraints scripts/check.sh
git commit -m "test(constraints): Aethoria end-to-end, cross-target golden, 5k-constraint budget" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Spec coverage (self-review)

| Spec section | Task |
|---|---|
| §3 constraints + common fields (`strength`, `tolerance`, `keep_*`, `name`) | 3 (validators), 4 (tolerance/keep read in evaluate) |
| §4.1 findings (support/issue/consequence, params, related, suggestions) | 1 |
| §4.2 score, grade, presets, realism on planet, status, keep-by-code | 1, 4 |
| §4.3 verdict, counts, report hash | 1 |
| §5.1–5.2 checker trait, registry, duplicates | 4 |
| §5.3 context, terrain, `UnknownTerrain` | 4 |
| §5.4 evaluation order, clamping, sorting | 4 |
| §5.5 templates with fallback | 4 |
| §6 kind catalog, validators, `Value::Entity` | 1, 3 |
| §7 six starter checkers (incl. spherical point-in-polygon) | 2, 6, 7, 8 |
| §8 actions | 5 |
| §9 errors | 1 |
| §10 grading properties, per-checker tests, Aethoria, end-to-end, golden, perf | 1, 6–9 |
