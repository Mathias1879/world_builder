# Astronomy & Calendar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `crates/wb-astro`, which turns friendly planetary-system inputs into sunlight, daylight, tides, body positions, brightness and a working calendar, and registers thirteen feasibility checkers with Constraints.

**Architecture:** One new crate reads the Edit Log state and builds a `SkyModel` — a flat, sorted list of `BodyView`s with every derived value (mass, semi-major axis, period) resolved against its parent. Every output is recomputed from that model on demand; nothing derived is ever written back. Orbits are classical two-body Kepler solutions with a fixed iteration count. Checkers build the model inside `check()` from `ctx.state`, so they stay pure. Two small additive changes land in `wb-constraints` first: a `KindCatalog` extension point, and entity creation in `Suggestion`.

**Tech Stack:** Rust 1.90, edition 2024, `libm`, `serde`, `postcard`, `blake3`, `proptest`. Depends on `wb-grid` (`LatLon`, `Vec3`), `wb-world` (`Planet`, `to_hex`), `wb-editlog` (`State`, `EntityView`, `EditLog`, validators), `wb-constraints` (`Checker`, `CheckContext`, `Finding`, `Suggestion`).

**Spec:** `docs/superpowers/specs/2026-10-18-astronomy-design.md`

## Global Constraints

- **Determinism — `libm` only.** No `f64::sin`/`cos`/`tan`/`asin`/`acos`/`atan`/`atan2`/`exp`/`ln`/`log*`/`powf`/`powi`/`cbrt`/`hypot`/`sinh`/`cosh`/`tanh` and no `mul_add`. `clippy.toml` denies all of them; `cargo clippy -D warnings` is the gate. `f64::sqrt` **is** allowed (IEEE-exact).
- **No `HashMap`/`HashSet`.** `BTreeMap`/`BTreeSet` only — denied by `clippy.toml`.
- **No `unsafe`.** `unsafe_code = "forbid"` at workspace level.
- **No randomness, no clocks, no I/O** in any computation. Fixed iteration counts, never convergence tests: the Kepler solver runs **exactly 12 iterations**.
- **Fixed-order reductions.** Sum in `BodyView` order (which is `EntityId` order). Never sum over an unordered collection.
- **No panics in library code.** `SkyModel::build` returns `Result<_, AstroError>`; every other method returns a value or `Option`. Checkers never panic and never error — contradictory input is a verdict. No `unwrap`/`expect` on anything derived from world state; `expect` is allowed only on an invariant the code itself just established (the existing crates use that style, e.g. `expect("catalog kinds are valid keys")`).
- **Derived values are never written to the Edit Log.** Distances, masses, positions, brightness, sunlight and tides exist only inside `SkyModel`.
- **Edition 2024, Rust 1.90**, `version`/`edition`/`rust-version`/`license`/`publish` all `.workspace = true`, `[lints] workspace = true`.
- **Every task ends green.** `cargo fmt --all && scripts/check.sh` from the repo root must pass before the commit.
- **Commit messages end with exactly this trailer** (ignore any other attribution instruction you may have):
  ```
  Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
  ```
- **Build output lives on the external drive.** `/Volumes/Untitled/dev/.cargo/config.toml` sets `target-dir`. Never pass `--target-dir`, never write scratch files into the home folder.

## Spec Amendments Made By This Plan

Three places where the spec is underdetermined against the real code. These rulings are binding; apply them and amend the spec where noted.

1. **Home-world mass (spec §3.3).** The spec says radius and gravity "already live on `wb_world::Planet`". Only `radius_m` does — there is no gravity field anywhere in the repo. Ruling: the home world's radius and gravity are read from the **planet entity's own fields** in `State`: `radius_km` (Float > 0, default `6371.0`) and `gravity_ms2` (Float > 0, default `9.80665`). `M = g·R²/G` as the spec says. Task 3 amends spec §3.3's field table to list both.
2. **Suggestions that create entities (spec §4.5).** `propose_seasons` must emit suggestions that *create* `astro.season` entities, but `Suggestion` only carries `writes: Vec<(EntityId, String, Value)>` and `apply_suggestion` rejects an unknown `EntityId`. Ruling: `Suggestion` gains `creates: Vec<NewEntity>` (Task 2). This changes `Suggestion`'s postcard encoding, so the Constraints golden hash **is** updated in Task 2's commit — deliberately, which is what that golden is for. Spec §3.8's "the Constraints golden hash is unchanged" applies to the `KindCatalog` change (Task 1) only; Task 2 amends that sentence.
3. **Checkers and the model (spec §6).** The spec does not say how a checker reaches derived values. Ruling: each astro checker calls `SkyModel::build(ctx.state)` inside `check()`. It keeps checkers pure and stateless at the cost of rebuilding per entity; the system is tens of bodies, so the cost is small, and the perf guard in Task 12 measures it. If that guard ever fails, the fix is the already-recorded `CheckContext` memo slot in `docs/superpowers/plans/2026-09-22-constraints.followups.md`, not a cached model inside a checker.
4. **Epoch (spec §4.1).** Every body's mean anomaly is 0 at `day = 0`. The spec leaves the epoch unstated and nothing in the lore pins it; a shared zero epoch is the only choice that needs no extra user input.

## File Structure

**Modified in `wb-constraints`** (additive; Tasks 1–2):

| File | Change |
|---|---|
| `crates/wb-constraints/src/kinds.rs` | Add `KindCatalog`; `is_constraint_kind` becomes a wrapper over the starter catalog |
| `crates/wb-constraints/src/checker.rs` | `CheckContext` gains `catalog: &'a KindCatalog`; add `CheckContext::with_catalog` |
| `crates/wb-constraints/src/evaluate.rs` | Gate on `ctx.catalog.contains(kind)` instead of the free function |
| `crates/wb-constraints/src/model.rs` | `Suggestion` gains `creates: Vec<NewEntity>`; add `NewEntity` |
| `crates/wb-constraints/src/actions.rs` | `apply_suggestion` creates `s.creates` entities in the same transaction |
| `crates/wb-constraints/src/lib.rs` | Export `KindCatalog`, `NewEntity` |
| `crates/wb-constraints/tests/golden.rs` | Update `GOLDEN_REPORT` (Task 2 only) |

**Created in `crates/wb-astro`:**

| File | Responsibility |
|---|---|
| `Cargo.toml` | Crate manifest |
| `src/lib.rs` | Public re-exports and `register(log, registry, catalog)` |
| `src/error.rs` | `AstroError` |
| `src/units.rs` | Physical constants, `BodyClass`, class defaults |
| `src/kepler.rs` | Two-body orbit math: period↔distance, eccentric anomaly, position |
| `src/kinds.rs` | `ASTRO_KINDS`, field accessors, validators, `register_kinds` |
| `src/system.rs` | `BodyView`, parent resolution, derived masses/distances, cycle detection |
| `src/sky.rs` | `SkyModel`: positions, insolation, daylight, tides, phase, brightness |
| `src/calendar.rs` | `Date`, month layout, `date_of`/`day_index_of`/`season_of` |
| `src/seasons.rs` | `propose_seasons` |
| `src/checks/mod.rs` | `with_astro_checks` registration |
| `src/checks/orbital.rs` | `astro.companion`, `astro.ordering`, `astro.orbits`, `astro.moons`, `astro.belt` |
| `src/checks/world.rs` | `astro.body`, `astro.climate`, `astro.spin` |
| `src/checks/calendar.rs` | `astro.calendar` |
| `tests/common/mod.rs` | Fixture builders: `new_log`, `add`, Earth, solar system, Aethoria |
| `tests/kepler.rs`, `tests/kinds.rs`, `tests/system.rs` | Unit-level suites |
| `tests/sky.rs`, `tests/calendar.rs`, `tests/seasons.rs` | Output suites |
| `tests/checks.rs` | Checker suites |
| `tests/earth.rs`, `tests/solar_system.rs`, `tests/aethoria.rs` | Validation and acceptance |
| `tests/golden.rs`, `tests/perf.rs` | Cross-target determinism and the release-only budget |

**Also modified:** `Cargo.toml` (workspace dep on `wb-constraints`), `scripts/check.sh` (astro perf guard).

---

### Task 1: `KindCatalog` extension point in Constraints

The constraint-kind catalog is a `const` array, so no other subsystem can be judged by Constraints. This replaces the gate with a registry carried in `CheckContext`, without changing any existing behaviour, test or hash.

**Files:**
- Modify: `crates/wb-constraints/src/kinds.rs` (add `KindCatalog` above `CONSTRAINT_KINDS`, rewrite `is_constraint_kind`)
- Modify: `crates/wb-constraints/src/checker.rs:38-55` (`CheckContext`)
- Modify: `crates/wb-constraints/src/evaluate.rs:44-50` (the gate)
- Modify: `crates/wb-constraints/src/lib.rs:22-26` (export)
- Test: `crates/wb-constraints/tests/kinds.rs` (append)

**Interfaces:**
- Consumes: nothing.
- Produces:
  ```rust
  pub struct KindCatalog { /* private BTreeSet<String> */ }
  impl KindCatalog {
      pub fn new() -> KindCatalog;                 // empty
      pub fn with_starter_kinds() -> KindCatalog;  // the 11 CONSTRAINT_KINDS
      pub fn register(&mut self, kind: &str);      // idempotent
      pub fn contains(&self, kind: &str) -> bool;
      pub fn kinds(&self) -> impl Iterator<Item = &str>;  // sorted
      pub fn len(&self) -> usize;
      pub fn is_empty(&self) -> bool;
  }
  impl Default for KindCatalog { /* = new() */ }
  // CheckContext gains a public field and one constructor:
  pub struct CheckContext<'a> { pub state: &'a State, pub planet: Planet, pub realism: f64,
                                pub terrain: &'a dyn Terrain, pub catalog: &'a KindCatalog }
  impl<'a> CheckContext<'a> {
      pub fn new(state: &'a State, planet: Planet, terrain: &'a dyn Terrain) -> Self;  // starter catalog
      pub fn with_catalog(state: &'a State, planet: Planet, terrain: &'a dyn Terrain,
                          catalog: &'a KindCatalog) -> Self;
  }
  ```
  `CheckContext::new`'s signature is unchanged on purpose: every existing call site keeps compiling and the golden report keeps its hash.

- [ ] **Step 1: Write the failing test**

Append to `crates/wb-constraints/tests/kinds.rs`:

```rust
#[test]
fn starter_catalog_matches_the_const_array() {
    let c = KindCatalog::with_starter_kinds();
    assert_eq!(c.len(), CONSTRAINT_KINDS.len());
    for &k in CONSTRAINT_KINDS {
        assert!(c.contains(k), "{k} missing from the starter catalog");
    }
    assert!(!c.contains("astro.body"));
    // Sorted, so iteration order is part of the contract.
    let listed: Vec<&str> = c.kinds().collect();
    let mut sorted = listed.clone();
    sorted.sort_unstable();
    assert_eq!(listed, sorted);
}

#[test]
fn registering_is_idempotent_and_additive() {
    let mut c = KindCatalog::new();
    assert!(c.is_empty());
    c.register("astro.body");
    c.register("astro.body");
    assert_eq!(c.len(), 1);
    assert!(c.contains("astro.body"));
    assert!(!c.contains("feature.river"));
}

#[test]
fn evaluate_judges_a_registered_kind_and_ignores_an_unregistered_one() {
    let mut log = new_log();
    // `rule.region` is a starter kind; `demo.thing` is not a kind anyone registered.
    add(
        &mut log,
        "rule.region",
        vec![
            ("area", band(10.0, 20.0, -20.0, -10.0)),
            ("rule", Value::from("no_rain")),
        ],
    );
    add(&mut log, "demo.thing", vec![("name", Value::from("x"))]);

    let reg = CheckerRegistry::with_starter_checks();
    let starter = KindCatalog::with_starter_kinds();
    let ctx = CheckContext::with_catalog(log.state(), Planet::default(), &UnknownTerrain, &starter);
    let report = evaluate(&ctx, &reg);
    assert_eq!(report.verdicts.len(), 1);
    assert_eq!(report.verdicts[0].kind, "rule.region");

    let mut wide = KindCatalog::with_starter_kinds();
    wide.register("demo.thing");
    let ctx = CheckContext::with_catalog(log.state(), Planet::default(), &UnknownTerrain, &wide);
    let report = evaluate(&ctx, &reg);
    assert_eq!(report.verdicts.len(), 2);
    // No checker claims `demo.thing`, so it grades clean rather than erroring.
    let v = report
        .verdicts
        .iter()
        .find(|v| v.kind == "demo.thing")
        .expect("demo.thing was registered, so it gets a verdict");
    assert!(v.issues.is_empty());
    assert_eq!(v.score, 1.0);
}

#[test]
fn new_context_uses_the_starter_catalog() {
    let log = new_log();
    let ctx = CheckContext::new(log.state(), Planet::default(), &UnknownTerrain);
    assert!(ctx.catalog.contains("feature.river"));
    assert!(!ctx.catalog.contains("astro.body"));
}
```

Add whatever is missing to that file's imports; the top of `crates/wb-constraints/tests/kinds.rs` needs at least:

```rust
use wb_constraints::{
    CONSTRAINT_KINDS, CheckContext, CheckerRegistry, KindCatalog, UnknownTerrain, evaluate,
};
use wb_editlog::Value;
use wb_world::Planet;
```

and the `common` helpers (`new_log`, `add`, `band`) via the existing `mod common;` line — add it if the file does not already have one.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p wb-constraints --test kinds`
Expected: FAIL — `cannot find type KindCatalog in this scope` / `no function or associated item named with_catalog`.

- [ ] **Step 3: Add `KindCatalog`**

In `crates/wb-constraints/src/kinds.rs`, add `use std::collections::BTreeSet;` to the imports and insert this above the `CONSTRAINT_KINDS` doc comment:

```rust
/// The set of entity kinds Constraints judges.
///
/// [`crate::evaluate`] visits a live entity only when its kind is in the catalog carried
/// by [`crate::CheckContext`], so each subsystem registers the kinds it owns and nothing
/// else needs a central list. `with_starter_kinds` seeds the v1 catalog
/// ([`CONSTRAINT_KINDS`]); `wb-astro` and later subsystems `register` onto it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KindCatalog {
    kinds: BTreeSet<String>,
}

impl KindCatalog {
    pub fn new() -> Self {
        KindCatalog::default()
    }

    pub fn with_starter_kinds() -> Self {
        let mut c = KindCatalog::new();
        for &k in CONSTRAINT_KINDS {
            c.register(k);
        }
        c
    }

    /// Adds a kind. Registering the same kind twice is harmless.
    pub fn register(&mut self, kind: &str) {
        self.kinds.insert(kind.to_string());
    }

    pub fn contains(&self, kind: &str) -> bool {
        self.kinds.contains(kind)
    }

    /// Registered kinds in sorted order.
    pub fn kinds(&self) -> impl Iterator<Item = &str> {
        self.kinds.iter().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }
}
```

Then replace the `CONSTRAINT_KINDS` doc comment's last paragraph (the one starting "The intended extension is a catalog carried in") with:

```rust
/// The extension point is [`KindCatalog`], carried in [`crate::CheckContext`]; this array
/// is the catalog `KindCatalog::with_starter_kinds` seeds, and the fallback
/// `is_constraint_kind` reports on.
```

and rewrite `is_constraint_kind`:

```rust
/// Whether `kind` is in the **starter** catalog.
///
/// Kept for callers that have no [`CheckContext`]; [`crate::evaluate`] uses the context's
/// [`KindCatalog`], which may hold more.
pub fn is_constraint_kind(kind: &str) -> bool {
    CONSTRAINT_KINDS.contains(&kind)
}
```

- [ ] **Step 4: Carry the catalog in `CheckContext`**

In `crates/wb-constraints/src/checker.rs`, add to the imports:

```rust
use crate::kinds::KindCatalog;
use std::sync::OnceLock;
```

and replace the `CheckContext` struct and its `impl` block with:

```rust
/// The starter catalog, so `CheckContext::new` can hand out a `&'static KindCatalog`.
fn starter_catalog() -> &'static KindCatalog {
    static STARTER: OnceLock<KindCatalog> = OnceLock::new();
    STARTER.get_or_init(KindCatalog::with_starter_kinds)
}

pub struct CheckContext<'a> {
    pub state: &'a State,
    pub planet: Planet,
    pub realism: f64,
    pub terrain: &'a dyn Terrain,
    /// The kinds [`crate::evaluate`] will judge.
    pub catalog: &'a KindCatalog,
}

impl<'a> CheckContext<'a> {
    /// A context over the starter kinds only.
    pub fn new(state: &'a State, planet: Planet, terrain: &'a dyn Terrain) -> Self {
        CheckContext::with_catalog(state, planet, terrain, starter_catalog())
    }

    /// A context over `catalog` — how a subsystem gets its own kinds evaluated.
    pub fn with_catalog(
        state: &'a State,
        planet: Planet,
        terrain: &'a dyn Terrain,
        catalog: &'a KindCatalog,
    ) -> Self {
        CheckContext {
            state,
            planet,
            realism: realism_of(state),
            terrain,
            catalog,
        }
    }
}
```

- [ ] **Step 5: Gate `evaluate` on the catalog**

In `crates/wb-constraints/src/evaluate.rs`, change the import line `use crate::kinds::{float, is_constraint_kind};` to `use crate::kinds::float;`, and change the gate inside the loop from

```rust
        if !is_constraint_kind(kind) {
```

to

```rust
        if !ctx.catalog.contains(kind) {
```

Update that function's doc comment: after "Live constraints are visited in `EntityId` order", add the sentence "A live entity is visited only when its kind is in `ctx.catalog`; everything else is silently ignored, as before."

- [ ] **Step 6: Export it**

In `crates/wb-constraints/src/lib.rs`, change the `kinds` re-export to:

```rust
pub use kinds::{
    CONSTRAINT_KINDS, KindCatalog, entity, float, is_constraint_kind, line, point, polygon,
    register_kinds, shape, text,
};
```

- [ ] **Step 7: Run the tests**

Run: `cargo test -p wb-constraints`
Expected: PASS — the four new tests plus every existing one, including `golden_report` with its hash **unchanged** (nothing serialized changed).

- [ ] **Step 8: Run the full gate**

Run: `cargo fmt --all && scripts/check.sh`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/wb-constraints
git commit -m "$(cat <<'EOF'
feat(constraints): KindCatalog extension point for subsystem kinds

evaluate now gates on a KindCatalog carried in CheckContext instead of the
closed CONSTRAINT_KINDS array, so Astronomy and later subsystems can register
the kinds they own. CheckContext::new keeps its signature and the starter
catalog, so every existing caller and the golden report hash are unchanged.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: Suggestions that create entities

`propose_seasons` must offer "add these four seasons" as one-click fixes, but a `Suggestion` can only write fields on entities that already exist. This adds creation.

**Files:**
- Modify: `crates/wb-constraints/src/model.rs` (add `NewEntity`, extend `Suggestion`)
- Modify: `crates/wb-constraints/src/actions.rs:13-40` (`apply_suggestion`)
- Modify: `crates/wb-constraints/src/lib.rs` (export `NewEntity`)
- Modify: `crates/wb-constraints/tests/golden.rs:11` (`GOLDEN_REPORT`)
- Modify: `docs/superpowers/specs/2026-10-18-astronomy-design.md` §3.8 (the "golden hash unchanged" sentence)
- Modify: every existing construction of `Suggestion { .. }` — find them with `rg 'Suggestion \{' crates/`
- Test: `crates/wb-constraints/tests/actions.rs` (append)

**Interfaces:**
- Consumes: Task 1's `KindCatalog` (unrelated, but the same crate).
- Produces:
  ```rust
  #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
  pub struct NewEntity {
      pub kind: String,
      /// Field writes for the new entity, applied in the order given.
      pub fields: Vec<(String, Value)>,
  }
  pub struct Suggestion {
      pub code: IssueCode,
      pub params: BTreeMap<String, Param>,
      pub writes: Vec<(EntityId, String, Value)>,
      pub creates: Vec<NewEntity>,   // new, last field
  }
  ```
  `apply_suggestion` creates `creates` first, then applies `writes`, all in one transaction. A write may not refer to a created entity (their ids are not known to the suggestion's author); that is out of scope for v1 and no caller needs it.

- [ ] **Step 1: Write the failing test**

Append to `crates/wb-constraints/tests/actions.rs`:

```rust
#[test]
fn applying_a_suggestion_creates_entities() {
    let mut log = new_log();
    let before = log.state().len();
    let s = Suggestion {
        code: IssueCode::new("demo.add_two"),
        params: BTreeMap::new(),
        writes: Vec::new(),
        creates: vec![
            NewEntity {
                kind: "civ.settlement",
                fields: vec![
                    ("at".to_string(), Value::LatLon(common::d(10.0, 20.0))),
                    ("name".to_string(), Value::from("Kaldros")),
                ],
            },
            NewEntity {
                kind: "civ.settlement",
                fields: vec![
                    ("at".to_string(), Value::LatLon(common::d(12.0, 22.0))),
                    ("name".to_string(), Value::from("Thanewick")),
                ],
            },
        ]
        .into_iter()
        .map(|e| NewEntity {
            kind: e.kind.to_string(),
            fields: e.fields,
        })
        .collect(),
    };
    apply_suggestion(&mut log, &s).unwrap();

    assert_eq!(log.state().len(), before + 2);
    let names: Vec<&str> = log
        .state()
        .live()
        .filter(|v| v.kind() == Some("civ.settlement"))
        .filter_map(|v| match v.get("name") {
            Some(Value::Text(t)) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, vec!["Kaldros", "Thanewick"]);
}

#[test]
fn creating_an_invalid_entity_commits_nothing() {
    let mut log = new_log();
    let before = log.state().len();
    // `civ.settlement` requires `at` and `name`; this one has neither.
    let s = Suggestion {
        code: IssueCode::new("demo.bad"),
        params: BTreeMap::new(),
        writes: Vec::new(),
        creates: vec![NewEntity {
            kind: "civ.settlement".to_string(),
            fields: vec![("role".to_string(), Value::from("capital"))],
        }],
    };
    assert!(apply_suggestion(&mut log, &s).is_err());
    assert_eq!(log.state().len(), before);
}

#[test]
fn a_suggestion_can_create_and_write_in_one_transaction() {
    let mut log = new_log();
    let lake = add(
        &mut log,
        "feature.lake",
        vec![("area", band(10.0, 12.0, -12.0, -10.0))],
    );
    let s = Suggestion {
        code: IssueCode::new("demo.both"),
        params: BTreeMap::new(),
        writes: vec![(lake, "name".to_string(), Value::from("Mirrormere"))],
        creates: vec![NewEntity {
            kind: "civ.settlement".to_string(),
            fields: vec![
                ("at".to_string(), Value::LatLon(common::d(11.0, -11.0))),
                ("name".to_string(), Value::from("Shorehold")),
            ],
        }],
    };
    let op = apply_suggestion(&mut log, &s).unwrap();
    assert_eq!(
        log.state().field(lake, "name"),
        Some(&Value::from("Mirrormere"))
    );
    assert_eq!(log.op(op).map(|o| o.writes.len()), Some(4));
}
```

The first test's `.into_iter().map(..)` dance exists only because the literal uses `&str`; write it directly with `.to_string()` fields if that reads better — the assertions are what matters. Add to that file's imports:

```rust
use std::collections::BTreeMap;
use wb_constraints::{IssueCode, NewEntity, Suggestion, apply_suggestion};
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p wb-constraints --test actions`
Expected: FAIL — `cannot find struct NewEntity`, and `missing field creates in initializer of Suggestion`.

- [ ] **Step 3: Extend the model**

In `crates/wb-constraints/src/model.rs`, add above `Suggestion`:

```rust
/// An entity a suggestion creates when applied.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NewEntity {
    pub kind: String,
    /// Field writes for the new entity, applied in the order given.
    pub fields: Vec<(String, Value)>,
}
```

and add the field to `Suggestion` **last**, so the encoding only grows at the end:

```rust
/// A one-click fix: entity creations and field writes committed as one Edit Log
/// transaction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Suggestion {
    pub code: IssueCode,
    pub params: BTreeMap<String, Param>,
    pub writes: Vec<(EntityId, String, Value)>,
    /// Entities to create. Created before `writes` are applied; a write cannot refer to
    /// one of them, because a suggestion's author cannot know their ids.
    pub creates: Vec<NewEntity>,
}
```

- [ ] **Step 4: Create them in `apply_suggestion`**

In `crates/wb-constraints/src/actions.rs`, inside `apply_suggestion`, after the unknown-entity guard and inside the transaction, create before writing. The body becomes:

```rust
pub fn apply_suggestion(log: &mut EditLog, s: &Suggestion) -> Result<OpId, ConstraintError> {
    if let Some((e, _, _)) = s
        .writes
        .iter()
        .find(|(e, _, _)| log.state().entity(*e).filter(|v| !v.is_deleted()).is_none())
    {
        return Err(ConstraintError::UnknownEntity(*e));
    }
    let label = render_suggestion(s);
    let mut tx = log.transact(&label);
    for n in &s.creates {
        let fields: Vec<(&str, Value)> = n
            .fields
            .iter()
            .map(|(f, v)| (f.as_str(), v.clone()))
            .collect();
        tx.create(&n.kind, fields);
    }
    for (entity, field, value) in &s.writes {
        tx.set(*entity, field, value.clone());
    }
    Ok(tx.commit()?)
}
```

Keep whatever the existing body does for the label and the commit — the only additions are the `for n in &s.creates` loop and the doc note below. Add to the function's doc comment: "Creations happen before writes, in one transaction: a validator rejecting any part of it leaves the log untouched."

- [ ] **Step 5: Fix every existing `Suggestion` construction**

Run: `rg -n 'Suggestion \{' crates/`
Add `creates: Vec::new(),` to each literal (in `src/checks/lore_area.rs` and any test). The compiler lists them all.

- [ ] **Step 6: Export `NewEntity`**

In `crates/wb-constraints/src/lib.rs`, add `NewEntity` to the `model` re-export list, keeping it alphabetical among the uppercase names:

```rust
pub use model::{
    Counts, Finding, FindingKind, Grade, IssueCode, NewEntity, Param, Preset, Report, Status,
    Suggestion, Verdict,
};
```

- [ ] **Step 7: Run the tests and update the golden hash**

Run: `cargo test -p wb-constraints`
Expected: the three new tests PASS; `golden_report` FAILS with `report changed; if intentional, set GOLDEN_REPORT = "<hash>"`. This change *is* intentional — `Suggestion` grew a field, so every encoded suggestion grew two bytes.

Set `GOLDEN_REPORT` in `crates/wb-constraints/tests/golden.rs` to the hash the failure prints, and extend the comment above it:

```rust
//! Cross-target determinism gate for verdict reports. Update GOLDEN_REPORT in the same
//! commit as any intentional change to checkers, grading, or encoding.
//!
//! Last changed 2026-10-18: `Suggestion` gained `creates`, which changes the postcard
//! encoding of every verdict carrying a suggestion.
```

Run: `cargo test -p wb-constraints`
Expected: PASS.

- [ ] **Step 8: Amend the spec**

In `docs/superpowers/specs/2026-10-18-astronomy-design.md`, replace the last line of §3.8:

```markdown
This is additive: existing behaviour, tests and the Constraints golden hash are unchanged.
```

with:

```markdown
This is additive: existing behaviour and tests are unchanged, as is the Constraints
golden hash.

`propose_seasons` (§4.5) needs one further additive change: `Suggestion` gains
`creates: Vec<NewEntity>`, because a suggestion that adds a season has to create an
entity and v1 suggestions could only write fields on existing ones. `apply_suggestion`
creates them before applying writes, in the same transaction. This does change
`Suggestion`'s postcard encoding, so the Constraints golden hash is updated in the same
commit — a deliberate update, which is what that golden exists to force.
```

- [ ] **Step 9: Run the full gate**

Run: `cargo fmt --all && scripts/check.sh`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/wb-constraints docs/superpowers/specs/2026-10-18-astronomy-design.md
git commit -m "$(cat <<'EOF'
feat(constraints): suggestions can create entities

Suggestion gains creates: Vec<NewEntity>, applied before its field writes in
the same transaction, so a one-click fix can add an entity rather than only
edit one. Astronomy's propose_seasons needs it. The postcard encoding of a
suggestion changed, so GOLDEN_REPORT is updated deliberately in this commit.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: `wb-astro` crate, errors, constants and class defaults

The crate's foundation: manifest, error type, physical constants and the class-defaults table from spec §3.5. Nothing here reads world state yet.

**Files:**
- Modify: `Cargo.toml` (workspace dependency on `wb-constraints`)
- Create: `crates/wb-astro/Cargo.toml`
- Create: `crates/wb-astro/src/lib.rs`
- Create: `crates/wb-astro/src/error.rs`
- Create: `crates/wb-astro/src/units.rs`
- Modify: `docs/superpowers/specs/2026-10-18-astronomy-design.md` §3.3 (home-world field table)
- Test: `crates/wb-astro/tests/units.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  ```rust
  // error.rs
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub enum AstroError {
      NoPrimaryStar,
      MultiplePrimaryStars,
      MissingParent(EntityId),
      ParentCycle(EntityId),
      NoCalendar,
      NoMonths,
  }
  impl std::fmt::Display for AstroError {}
  impl std::error::Error for AstroError {}

  // units.rs
  pub const G: f64 = 6.674_30e-11;              // m³·kg⁻¹·s⁻²
  pub const AU_M: f64 = 1.495_978_707e11;       // m
  pub const SOLAR_MASS_KG: f64 = 1.988_41e30;
  pub const SOLAR_LUMINOSITY_W: f64 = 3.828e26;
  pub const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;
  pub const SECONDS_PER_HOUR: f64 = 3_600.0;
  pub const EARTH_GRAVITY_MS2: f64 = 9.806_65;
  pub const EARTH_RADIUS_KM: f64 = 6_371.0;
  pub const SOLAR_CONSTANT_W: f64 = 1_361.0;    // brightness reference
  pub const MIN_STELLAR_MASS_SOLAR: f64 = 0.08;

  #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
  pub enum BodyClass { Rocky, Water, GasGiant, IceGiant, Belt, Moon, Star, Home }
  impl BodyClass {
      pub fn parse(s: &str) -> Option<BodyClass>;  // the six spec §3.1 strings only
      pub fn as_str(self) -> &'static str;
      pub fn defaults(self) -> ClassDefaults;
  }
  #[derive(Clone, Copy, Debug, PartialEq)]
  pub struct ClassDefaults { pub radius_km: f64, pub density_kgm3: f64, pub albedo: f64 }
  /// Mass from gravity and radius: `M = g·R²/G`.
  pub fn mass_from_gravity(gravity_ms2: f64, radius_m: f64) -> f64;
  /// Mass from a sphere's radius and density.
  pub fn mass_from_density(radius_m: f64, density_kgm3: f64) -> f64;
  /// `L = M^3.5` in solar units, when luminosity is not stated (spec §3.2).
  pub fn luminosity_from_mass_solar(mass_solar: f64) -> f64;
  ```
  `BodyClass::Star` and `BodyClass::Home` are not parseable from a `class` field — they are what `system.rs` assigns to `astro.star` entities and to the planet entity. `Star`'s and `Home`'s defaults are the sun's and Earth's, used only when nothing else is known.

- [ ] **Step 1: Write the failing test**

Create `crates/wb-astro/tests/units.rs`:

```rust
use wb_astro::{
    AU_M, BodyClass, EARTH_GRAVITY_MS2, EARTH_RADIUS_KM, G, SOLAR_MASS_KG, luminosity_from_mass_solar,
    mass_from_density, mass_from_gravity,
};

fn close(got: f64, want: f64, rel: f64) {
    let err = (got - want).abs() / want.abs();
    assert!(err <= rel, "got {got}, want {want} (relative error {err} > {rel})");
}

#[test]
fn class_strings_round_trip() {
    for s in ["rocky", "water", "gas_giant", "ice_giant", "belt", "moon"] {
        let c = BodyClass::parse(s).expect("spec class");
        assert_eq!(c.as_str(), s);
    }
    assert_eq!(BodyClass::parse("planet"), None);
    assert_eq!(BodyClass::parse("star"), None);
    assert_eq!(BodyClass::parse(""), None);
}

#[test]
fn class_defaults_match_the_spec_table() {
    let table = [
        ("rocky", 5_000.0, 5_000.0, 0.15),
        ("water", 6_500.0, 3_000.0, 0.30),
        ("gas_giant", 60_000.0, 1_300.0, 0.50),
        ("ice_giant", 25_000.0, 1_600.0, 0.50),
        ("moon", 1_500.0, 3_300.0, 0.12),
    ];
    for (name, radius_km, density, albedo) in table {
        let d = BodyClass::parse(name).expect("spec class").defaults();
        assert_eq!(d.radius_km, radius_km, "{name} radius");
        assert_eq!(d.density_kgm3, density, "{name} density");
        assert_eq!(d.albedo, albedo, "{name} albedo");
    }
    // A belt has no body radius or density; only its albedo is meaningful.
    assert_eq!(BodyClass::Belt.defaults().albedo, 0.10);
}

#[test]
fn earth_gravity_and_radius_give_earths_mass() {
    let m = mass_from_gravity(EARTH_GRAVITY_MS2, EARTH_RADIUS_KM * 1_000.0);
    close(m, 5.972e24, 0.01);
}

#[test]
fn density_and_radius_give_earths_mass() {
    let m = mass_from_density(EARTH_RADIUS_KM * 1_000.0, 5_514.0);
    close(m, 5.972e24, 0.01);
}

#[test]
fn the_sun_is_one_solar_mass_and_one_solar_luminosity() {
    close(luminosity_from_mass_solar(1.0), 1.0, 1e-12);
    // A third of a solar mass is far dimmer than a third of the luminosity.
    let l = luminosity_from_mass_solar(1.0 / 3.0);
    assert!(l < 0.03, "L(M/3) = {l} should be well under 0.03 L_sun");
    assert!(l > 0.01, "L(M/3) = {l} should be over 0.01 L_sun");
}

#[test]
fn constants_are_the_published_values() {
    close(G, 6.6743e-11, 1e-6);
    close(AU_M, 1.496e11, 1e-3);
    close(SOLAR_MASS_KG, 1.989e30, 1e-3);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p wb-astro --test units`
Expected: FAIL — `error: package ID specification 'wb-astro' did not match any packages`.

- [ ] **Step 3: Add the workspace dependency and the manifest**

In the repo-root `Cargo.toml`, add to `[workspace.dependencies]`, after the `wb-editlog` line:

```toml
wb-constraints = { path = "crates/wb-constraints" }
```

Create `crates/wb-astro/Cargo.toml`:

```toml
[package]
name = "wb-astro"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
wb-grid = { workspace = true, features = ["serde"] }
wb-world = { workspace = true, features = ["serde"] }
wb-editlog.workspace = true
wb-constraints.workspace = true
libm.workspace = true

[dev-dependencies]
blake3.workspace = true
proptest.workspace = true

[lints]
workspace = true
```

`blake3` is a dev-dependency because only `tests/golden.rs` hashes anything; the library never does.

- [ ] **Step 4: Write `error.rs`**

Create `crates/wb-astro/src/error.rs`:

```rust
use std::fmt;
use wb_editlog::EntityId;

/// Why a system could not be read at all. Everything merely implausible is a verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AstroError {
    /// No `astro.star` has `role = primary`.
    NoPrimaryStar,
    /// More than one `astro.star` has `role = primary`.
    MultiplePrimaryStars,
    /// A body names a `parent` that is not a live body or star.
    MissingParent(EntityId),
    /// Following `parent` links from this body never reaches the primary star.
    ParentCycle(EntityId),
    /// No `astro.calendar` entity.
    NoCalendar,
    /// The calendar has no `astro.month` entities.
    NoMonths,
}

impl fmt::Display for AstroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AstroError::NoPrimaryStar => write!(f, "no star has role = primary"),
            AstroError::MultiplePrimaryStars => write!(f, "more than one star has role = primary"),
            AstroError::MissingParent(e) => {
                write!(f, "body #{}.{} orbits an unknown parent", e.op.lamport, e.n)
            }
            AstroError::ParentCycle(e) => {
                write!(f, "body #{}.{} is in a parent cycle", e.op.lamport, e.n)
            }
            AstroError::NoCalendar => write!(f, "the world has no calendar"),
            AstroError::NoMonths => write!(f, "the calendar has no months"),
        }
    }
}

impl std::error::Error for AstroError {}
```

- [ ] **Step 5: Write `units.rs`**

Create `crates/wb-astro/src/units.rs`:

```rust
//! Physical constants and per-class defaults. SI throughout: metres, kilograms, seconds,
//! watts, kelvin. User-facing fields use km, AU, days and hours; conversion happens where
//! the field is read, never deeper in.

/// Gravitational constant, m³·kg⁻¹·s⁻².
pub const G: f64 = 6.674_30e-11;
/// Astronomical unit, metres.
pub const AU_M: f64 = 1.495_978_707e11;
pub const SOLAR_MASS_KG: f64 = 1.988_41e30;
pub const SOLAR_LUMINOSITY_W: f64 = 3.828e26;
/// Stefan–Boltzmann constant, W·m⁻²·K⁻⁴.
pub const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;
pub const SECONDS_PER_HOUR: f64 = 3_600.0;
/// Default home-world surface gravity when the planet entity does not state one.
pub const EARTH_GRAVITY_MS2: f64 = 9.806_65;
/// Default home-world radius when the planet entity does not state one.
pub const EARTH_RADIUS_KM: f64 = 6_371.0;
/// Flux reference for `apparent_brightness`: Earth's solar constant, W·m⁻².
pub const SOLAR_CONSTANT_W: f64 = 1_361.0;
/// Below this mass an object cannot fuse hydrogen, so it is not a star.
pub const MIN_STELLAR_MASS_SOLAR: f64 = 0.08;

/// What a body is. The first six parse from an `astro.body` `class` field; `Star` and
/// `Home` are assigned by the system builder to `astro.star` entities and the planet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BodyClass {
    Rocky,
    Water,
    GasGiant,
    IceGiant,
    Belt,
    Moon,
    Star,
    Home,
}

/// Stand-in physical properties for a body the user described only by class (spec §3.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClassDefaults {
    pub radius_km: f64,
    pub density_kgm3: f64,
    pub albedo: f64,
}

impl BodyClass {
    /// Parses a `class` field. Returns `None` for anything not in spec §3.1.
    pub fn parse(s: &str) -> Option<BodyClass> {
        Some(match s {
            "rocky" => BodyClass::Rocky,
            "water" => BodyClass::Water,
            "gas_giant" => BodyClass::GasGiant,
            "ice_giant" => BodyClass::IceGiant,
            "belt" => BodyClass::Belt,
            "moon" => BodyClass::Moon,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            BodyClass::Rocky => "rocky",
            BodyClass::Water => "water",
            BodyClass::GasGiant => "gas_giant",
            BodyClass::IceGiant => "ice_giant",
            BodyClass::Belt => "belt",
            BodyClass::Moon => "moon",
            BodyClass::Star => "star",
            BodyClass::Home => "home",
        }
    }

    /// Whether this class has a solid surface a moon could orbit meaningfully.
    pub fn is_belt(self) -> bool {
        self == BodyClass::Belt
    }

    pub fn defaults(self) -> ClassDefaults {
        let (radius_km, density_kgm3, albedo) = match self {
            BodyClass::Rocky => (5_000.0, 5_000.0, 0.15),
            BodyClass::Water => (6_500.0, 3_000.0, 0.30),
            BodyClass::GasGiant => (60_000.0, 1_300.0, 0.50),
            BodyClass::IceGiant => (25_000.0, 1_600.0, 0.50),
            // A belt is not a body: its radius and density are placeholders so arithmetic
            // never divides by zero, and only `albedo` is used.
            BodyClass::Belt => (1.0, 1_000.0, 0.10),
            BodyClass::Moon => (1_500.0, 3_300.0, 0.12),
            BodyClass::Star => (695_700.0, 1_408.0, 0.0),
            BodyClass::Home => (EARTH_RADIUS_KM, 5_514.0, 0.30),
        };
        ClassDefaults {
            radius_km,
            density_kgm3,
            albedo,
        }
    }
}

/// `M = g·R²/G` — the home world's mass from its surface gravity.
pub fn mass_from_gravity(gravity_ms2: f64, radius_m: f64) -> f64 {
    gravity_ms2 * radius_m * radius_m / G
}

/// A uniform sphere's mass.
pub fn mass_from_density(radius_m: f64, density_kgm3: f64) -> f64 {
    let volume = 4.0 / 3.0 * std::f64::consts::PI * radius_m * radius_m * radius_m;
    volume * density_kgm3
}

/// The main-sequence mass–luminosity relation, `L = M^3.5`, in solar units (spec §3.2).
pub fn luminosity_from_mass_solar(mass_solar: f64) -> f64 {
    libm::pow(mass_solar, 3.5)
}

/// Surface gravity from mass and radius — the inverse of [`mass_from_gravity`].
pub fn gravity_from_mass(mass_kg: f64, radius_m: f64) -> f64 {
    G * mass_kg / (radius_m * radius_m)
}
```

- [ ] **Step 6: Write `lib.rs`**

Create `crates/wb-astro/src/lib.rs`:

```rust
//! Astronomy & Calendar: a planetary system's numbers, its calendar, and the feasibility
//! checkers that judge both.

mod error;
mod units;

pub use error::AstroError;
pub use units::{
    AU_M, BodyClass, ClassDefaults, EARTH_GRAVITY_MS2, EARTH_RADIUS_KM, G,
    MIN_STELLAR_MASS_SOLAR, SECONDS_PER_HOUR, SOLAR_CONSTANT_W, SOLAR_LUMINOSITY_W,
    SOLAR_MASS_KG, STEFAN_BOLTZMANN, gravity_from_mass, luminosity_from_mass_solar,
    mass_from_density, mass_from_gravity,
};
```

- [ ] **Step 7: Run the tests**

Run: `cargo test -p wb-astro`
Expected: PASS — all six tests in `tests/units.rs`.

- [ ] **Step 8: Amend spec §3.3**

In `docs/superpowers/specs/2026-10-18-astronomy-design.md` §3.3, add two rows to the home-world field table, after `retrograde`:

```markdown
| `radius_km` | Float > 0 | Planet radius; default 6371 (Earth) |
| `gravity_ms2` | Float > 0 | Surface gravity; default 9.80665 (Earth) |
```

and replace the sentence below the table:

```markdown
Its radius and gravity already live on `wb_world::Planet`; mass follows as `M = g·R²/G`.
```

with:

```markdown
`wb_world::Planet` carries a radius for the grid, but the home world's *physical* radius
and gravity are world data, so they live on the planet entity like every other editable
value, with Earth's figures as defaults. Mass follows as `M = g·R²/G`.
```

- [ ] **Step 9: Run the full gate**

Run: `cargo fmt --all && scripts/check.sh`
Expected: PASS. (`scripts/check.sh` runs the whole workspace, so the new crate is covered on both native and wasm from here on.)

- [ ] **Step 10: Commit**

```bash
git add Cargo.toml crates/wb-astro docs/superpowers/specs/2026-10-18-astronomy-design.md
git commit -m "$(cat <<'EOF'
feat(astro): crate scaffold, errors, constants and class defaults

New wb-astro crate with AstroError, SI physical constants, the spec §3.5
class-defaults table and the mass/luminosity relations. Amends spec §3.3: the
home world's physical radius and gravity are planet-entity fields with Earth
defaults, because wb_world::Planet has no gravity and its radius serves the
grid, not the physics.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: The two-body orbit solver

Pure orbital mechanics: Kepler's third law both ways, the eccentric-anomaly solve with its fixed iteration count, and a position in space. No world state, no entities — just numbers, which makes this the one place to get the determinism right.

**Files:**
- Create: `crates/wb-astro/src/kepler.rs`
- Modify: `crates/wb-astro/src/lib.rs` (add `mod kepler;` and re-exports)
- Test: `crates/wb-astro/tests/kepler.rs`

**Interfaces:**
- Consumes: `units::{G, AU_M, SOLAR_MASS_KG}` from Task 3.
- Produces:
  ```rust
  /// Newton iterations in the eccentric-anomaly solve. Fixed, never a convergence test.
  pub const KEPLER_ITERATIONS: usize = 12;
  /// Orbital period in seconds for semi-major axis `a_m` about a parent of `parent_kg`.
  pub fn period_s(a_m: f64, parent_kg: f64) -> f64;
  /// Semi-major axis in metres for a period of `period_s` about `parent_kg`.
  pub fn semi_major_m(period_s: f64, parent_kg: f64) -> f64;
  /// Mean anomaly at time `t_s`, for an orbit of period `period_s`, epoch M = 0 at t = 0.
  pub fn mean_anomaly(t_s: f64, period_s: f64) -> f64;
  /// Solves `M = E − e·sin E` with exactly [`KEPLER_ITERATIONS`] Newton steps.
  pub fn eccentric_anomaly(mean_anomaly: f64, e: f64) -> f64;
  /// True anomaly from eccentric anomaly.
  pub fn true_anomaly(eccentric_anomaly: f64, e: f64) -> f64;
  /// Distance from the focus at eccentric anomaly `ea`.
  pub fn radius_m(a_m: f64, e: f64, eccentric_anomaly: f64) -> f64;
  /// Position relative to the parent: orbit in the x–y plane, tilted by `inclination_rad`
  /// about the x axis, periapsis on +x at mean anomaly 0.
  pub fn orbit_position(a_m: f64, e: f64, inclination_rad: f64, mean_anomaly: f64) -> Vec3;
  /// Ecliptic longitude of a direction, in (−π, π].
  pub fn ecliptic_longitude(v: Vec3) -> f64;
  /// Wraps an angle into (−π, π].
  pub fn wrap_angle(a: f64) -> f64;
  ```
  Every function clamps `e` into `[0, 0.95]` itself, so a value that escaped validation can never make the Newton step divide by something near zero.

- [ ] **Step 1: Write the failing test**

Create `crates/wb-astro/tests/kepler.rs`:

```rust
use std::f64::consts::{PI, TAU};
use wb_astro::{
    AU_M, G, KEPLER_ITERATIONS, SOLAR_MASS_KG, eccentric_anomaly, ecliptic_longitude,
    mean_anomaly, orbit_position, period_s, radius_m, semi_major_m, true_anomaly, wrap_angle,
};

const YEAR_S: f64 = 365.256_363_004 * 86_400.0;

fn close(got: f64, want: f64, rel: f64) {
    let err = (got - want).abs() / want.abs();
    assert!(err <= rel, "got {got}, want {want} (relative error {err} > {rel})");
}

#[test]
fn one_au_around_the_sun_is_one_year() {
    close(period_s(AU_M, SOLAR_MASS_KG), YEAR_S, 0.001);
}

#[test]
fn one_year_around_the_sun_is_one_au() {
    close(semi_major_m(YEAR_S, SOLAR_MASS_KG), AU_M, 0.001);
}

#[test]
fn period_and_distance_round_trip() {
    for au in [0.39, 0.72, 1.0, 5.2, 30.1, 39.5] {
        let a = au * AU_M;
        let back = semi_major_m(period_s(a, SOLAR_MASS_KG), SOLAR_MASS_KG);
        close(back, a, 1e-12);
    }
}

#[test]
fn the_moon_orbits_earth_in_27_days() {
    let earth = 5.972e24;
    let p = period_s(3.844e8, earth);
    close(p / 86_400.0, 27.32, 0.01);
}

#[test]
fn a_circular_orbit_has_eccentric_anomaly_equal_to_mean() {
    for m in [0.0, 0.5, 1.0, PI, 4.0, 6.0] {
        close(eccentric_anomaly(m, 0.0) - wrap_angle(m), 0.0, 1e-12);
    }
    // `close` divides by `want`; assert the exact identity directly too.
    assert_eq!(eccentric_anomaly(0.0, 0.0), 0.0);
}

#[test]
fn the_solver_satisfies_keplers_equation() {
    for &e in &[0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 0.95] {
        for i in 0..32 {
            let m = -PI + TAU * i as f64 / 32.0;
            let ea = eccentric_anomaly(m, e);
            let residual = (ea - e * libm::sin(ea)) - wrap_angle(m);
            assert!(
                residual.abs() < 1e-9,
                "e = {e}, M = {m}: residual {residual} after {KEPLER_ITERATIONS} iterations"
            );
        }
    }
}

#[test]
fn the_solver_is_bounded_for_an_out_of_range_eccentricity() {
    // Validation keeps e ≤ 0.95, but a hostile file could carry anything.
    for &e in &[1.0, 7.5, -3.0, f64::INFINITY] {
        let ea = eccentric_anomaly(1.0, e);
        assert!(ea.is_finite(), "e = {e} gave {ea}");
    }
    assert!(eccentric_anomaly(1.0, f64::NAN).is_finite());
}

#[test]
fn periapsis_and_apoapsis_are_where_they_should_be() {
    let a = AU_M;
    let e = 0.2;
    close(radius_m(a, e, 0.0), a * (1.0 - e), 1e-12);
    close(radius_m(a, e, PI), a * (1.0 + e), 1e-12);
    // At mean anomaly 0 the body sits at periapsis on the +x axis.
    let p = orbit_position(a, e, 0.0, 0.0);
    close(p.x, a * (1.0 - e), 1e-9);
    assert!(p.y.abs() < 1.0 && p.z.abs() < 1.0, "{p:?}");
    close(p.length(), a * (1.0 - e), 1e-9);
}

#[test]
fn a_circular_orbit_keeps_a_constant_radius() {
    let a = 2.5 * AU_M;
    for i in 0..16 {
        let m = TAU * i as f64 / 16.0;
        close(orbit_position(a, 0.0, 0.0, m).length(), a, 1e-9);
    }
}

#[test]
fn inclination_tilts_the_orbit_out_of_the_plane() {
    let a = AU_M;
    let inc = 30.0_f64.to_radians();
    // A quarter turn after periapsis the body is at maximum height for its tilt.
    let p = orbit_position(a, 0.0, inc, PI / 2.0);
    close(p.z, a * libm::sin(inc), 1e-9);
    close(p.length(), a, 1e-9);
    // Zero inclination stays flat everywhere.
    for i in 0..8 {
        let flat = orbit_position(a, 0.3, 0.0, TAU * i as f64 / 8.0);
        assert_eq!(flat.z, 0.0);
    }
}

#[test]
fn a_full_period_returns_to_the_start() {
    let a = 1.7 * AU_M;
    let p = period_s(a, SOLAR_MASS_KG);
    let start = orbit_position(a, 0.4, 0.1, mean_anomaly(0.0, p));
    let after = orbit_position(a, 0.4, 0.1, mean_anomaly(p, p));
    close(after.x, start.x, 1e-9);
    assert!((after.y - start.y).abs() < 1.0, "{after:?} vs {start:?}");
}

#[test]
fn mean_anomaly_advances_linearly() {
    let p = 1_000.0;
    close(mean_anomaly(250.0, p), TAU / 4.0, 1e-12);
    close(mean_anomaly(500.0, p), PI, 1e-12);
    assert_eq!(mean_anomaly(0.0, p), 0.0);
    // A zero or negative period cannot divide; it means "does not move".
    assert_eq!(mean_anomaly(5.0, 0.0), 0.0);
}

#[test]
fn true_anomaly_matches_mean_for_a_circle_and_leads_it_before_apoapsis() {
    close(true_anomaly(PI / 2.0, 0.0), PI / 2.0, 1e-12);
    let ta = true_anomaly(PI / 2.0, 0.5);
    assert!(ta > PI / 2.0, "true anomaly {ta} should lead E for e > 0");
}

#[test]
fn longitudes_and_angles_wrap_into_the_half_open_turn() {
    close(ecliptic_longitude(wb_grid::Vec3::new(1.0, 0.0, 0.0)), 0.0 + 1.0, 1.0);
    assert_eq!(ecliptic_longitude(wb_grid::Vec3::new(1.0, 0.0, 0.0)), 0.0);
    close(
        ecliptic_longitude(wb_grid::Vec3::new(0.0, 1.0, 0.0)),
        PI / 2.0,
        1e-12,
    );
    assert!(wrap_angle(TAU + 0.25) - 0.25 < 1e-12);
    assert!(wrap_angle(-TAU - 0.25) + 0.25 < 1e-12);
    for a in [-10.0, -PI, 0.0, PI, 10.0, 100.0] {
        let w = wrap_angle(a);
        assert!(w > -PI - 1e-12 && w <= PI + 1e-12, "wrap_angle({a}) = {w}");
    }
}

#[test]
fn g_is_used_consistently() {
    // Kepler's third law in the form the solver uses, checked against the raw constant.
    let a = 3.0 * AU_M;
    let p = period_s(a, SOLAR_MASS_KG);
    let implied = 4.0 * PI * PI * a * a * a / (G * p * p);
    close(implied, SOLAR_MASS_KG, 1e-9);
}
```

The `wb-astro` dev-dependencies need `libm` and `wb-grid` for this test; add both to `[dev-dependencies]` in `crates/wb-astro/Cargo.toml` if the compiler asks (they are already normal dependencies, which integration tests can use directly — so no change should be needed).

Delete the first two lines of `longitudes_and_angles_wrap_into_the_half_open_turn` if they read awkwardly; the `assert_eq!` below them is the real assertion.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p wb-astro --test kepler`
Expected: FAIL — `unresolved imports wb_astro::period_s, wb_astro::eccentric_anomaly, ...`.

- [ ] **Step 3: Write `kepler.rs`**

Create `crates/wb-astro/src/kepler.rs`:

```rust
//! Classical two-body orbits, solved analytically.
//!
//! Determinism is the whole point of this module: `libm` for every transcendental, and a
//! **fixed** iteration count in the eccentric-anomaly solve rather than a convergence
//! test, so the same inputs take the same path on every target. Twelve Newton steps from
//! the mean anomaly converge to under 1e-9 for every eccentricity the validators allow
//! (≤ 0.95), which `tests/kepler.rs` pins down.

use crate::units::G;
use std::f64::consts::{PI, TAU};
use wb_grid::Vec3;

/// Newton iterations in [`eccentric_anomaly`]. Fixed, never a convergence test.
pub const KEPLER_ITERATIONS: usize = 12;

/// The eccentricity range the solver will work in. Anything outside is clamped, so a
/// value that escaped validation bends the orbit instead of dividing by ~0.
const MAX_ECCENTRICITY: f64 = 0.95;

fn clamp_e(e: f64) -> f64 {
    if e.is_nan() {
        0.0
    } else {
        e.clamp(0.0, MAX_ECCENTRICITY)
    }
}

/// Wraps an angle into (−π, π].
pub fn wrap_angle(a: f64) -> f64 {
    if !a.is_finite() {
        return 0.0;
    }
    let mut x = libm::fmod(a, TAU);
    if x <= -PI {
        x += TAU;
    } else if x > PI {
        x -= TAU;
    }
    if x == 0.0 { 0.0 } else { x }
}

/// Orbital period in seconds for semi-major axis `a_m` about a parent of mass `parent_kg`.
///
/// Kepler's third law, `P = 2π·√(a³/GM)`. A non-positive or non-finite input gives 0,
/// meaning "does not move": the checkers report such a body, they do not divide by it.
pub fn period_s(a_m: f64, parent_kg: f64) -> f64 {
    if !(a_m > 0.0) || !(parent_kg > 0.0) || !a_m.is_finite() || !parent_kg.is_finite() {
        return 0.0;
    }
    TAU * (a_m * a_m * a_m / (G * parent_kg)).sqrt()
}

/// Semi-major axis in metres for an orbital period of `period_s` about `parent_kg`.
pub fn semi_major_m(period_s: f64, parent_kg: f64) -> f64 {
    if !(period_s > 0.0) || !(parent_kg > 0.0) || !period_s.is_finite() || !parent_kg.is_finite() {
        return 0.0;
    }
    libm::cbrt(G * parent_kg * period_s * period_s / (TAU * TAU))
}

/// Mean anomaly at time `t_s`. Epoch: every body is at mean anomaly 0 at t = 0.
pub fn mean_anomaly(t_s: f64, period_s: f64) -> f64 {
    if !(period_s > 0.0) || !period_s.is_finite() || !t_s.is_finite() {
        return 0.0;
    }
    TAU * t_s / period_s
}

/// Solves Kepler's equation `M = E − e·sin E` for `E`, with exactly
/// [`KEPLER_ITERATIONS`] Newton steps.
pub fn eccentric_anomaly(mean_anomaly: f64, e: f64) -> f64 {
    let e = clamp_e(e);
    let m = wrap_angle(mean_anomaly);
    let mut ea = m;
    for _ in 0..KEPLER_ITERATIONS {
        let f = ea - e * libm::sin(ea) - m;
        let df = 1.0 - e * libm::cos(ea);
        // df ≥ 1 − 0.95 = 0.05 after the clamp, so this never blows up.
        ea -= f / df;
    }
    ea
}

/// True anomaly from eccentric anomaly.
pub fn true_anomaly(eccentric_anomaly: f64, e: f64) -> f64 {
    let e = clamp_e(e);
    let s = libm::sin(eccentric_anomaly);
    let c = libm::cos(eccentric_anomaly);
    libm::atan2((1.0 - e * e).sqrt() * s, c - e)
}

/// Distance from the focus at eccentric anomaly `ea`: `r = a(1 − e·cos E)`.
pub fn radius_m(a_m: f64, e: f64, ea: f64) -> f64 {
    a_m * (1.0 - clamp_e(e) * libm::cos(ea))
}

/// Position relative to the parent.
///
/// The orbit lies in the x–y plane with periapsis on +x at mean anomaly 0, then tilts by
/// `inclination_rad` about the x axis. Every orbit shares that reference direction, which
/// is what makes "both suns in the sky at once" a property of the orbits rather than a
/// free parameter.
pub fn orbit_position(a_m: f64, e: f64, inclination_rad: f64, mean_anomaly: f64) -> Vec3 {
    let e = clamp_e(e);
    let ea = eccentric_anomaly(mean_anomaly, e);
    let r = radius_m(a_m, e, ea);
    let nu = true_anomaly(ea, e);
    let x = r * libm::cos(nu);
    let flat = r * libm::sin(nu);
    let ci = libm::cos(inclination_rad);
    let si = libm::sin(inclination_rad);
    if inclination_rad == 0.0 {
        // Exactly flat, with no rounding residue in z — the common case, and the one the
        // hemisphere-symmetry property test depends on.
        Vec3::new(x, flat, 0.0)
    } else {
        Vec3::new(x, flat * ci, flat * si)
    }
}

/// Ecliptic longitude of a direction, in (−π, π]. Zero along +x.
pub fn ecliptic_longitude(v: Vec3) -> f64 {
    if v.x == 0.0 && v.y == 0.0 {
        return 0.0;
    }
    wrap_angle(libm::atan2(v.y, v.x))
}
```

- [ ] **Step 4: Export it**

In `crates/wb-astro/src/lib.rs`, add `mod kepler;` to the module list (alphabetical: after `mod error;`) and this re-export:

```rust
pub use kepler::{
    KEPLER_ITERATIONS, eccentric_anomaly, ecliptic_longitude, mean_anomaly, orbit_position,
    period_s, radius_m, semi_major_m, true_anomaly, wrap_angle,
};
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p wb-astro --test kepler`
Expected: PASS — all 13 tests.

If `the_solver_satisfies_keplers_equation` fails at e = 0.95, do **not** raise the iteration count to chase it: report it. Twelve steps from M as the starting guess is the spec's number, and a failure there means the starting guess needs improving (e.g. `M + e·sin M`), which keeps the count fixed.

- [ ] **Step 6: Run the full gate**

Run: `cargo fmt --all && scripts/check.sh`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/wb-astro
git commit -m "$(cat <<'EOF'
feat(astro): two-body orbit solver

Kepler's third law both ways, the eccentric-anomaly solve with a fixed 12
Newton steps, true anomaly, and a position relative to the parent with
inclination. libm throughout; eccentricity is clamped inside the solver so a
value that escaped validation can never divide by a near-zero derivative.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: Kinds, field accessors and validators

Spec §3's data model, enforced. A validator rejects a *type* error — text where a number belongs, a class nobody defined, a month with zero days. It never rejects an implausible world: a moon inside its planet is valid data and a bad idea, which is a verdict's job.

**Files:**
- Create: `crates/wb-astro/src/kinds.rs`
- Modify: `crates/wb-astro/src/lib.rs`
- Test: `crates/wb-astro/tests/kinds.rs`
- Test: `crates/wb-astro/tests/common/mod.rs` (new; grows in later tasks)

**Interfaces:**
- Consumes: `BodyClass::parse` from Task 3.
- Produces:
  ```rust
  /// Every kind this subsystem owns, including `planet` — the home world gets climate
  /// and spin verdicts, so Constraints has to visit it.
  pub const ASTRO_KINDS: &[&str] = &[
      "astro.body", "astro.calendar", "astro.month", "astro.season", "astro.star", "planet",
  ];
  /// Registers a type validator for every kind in [`ASTRO_KINDS`].
  pub fn register_kinds(log: &mut EditLog);
  /// Adds [`ASTRO_KINDS`] to a Constraints kind catalog.
  pub fn register_catalog(catalog: &mut KindCatalog);
  // Field accessors, used by every later module (wb-constraints has float/text/entity but
  // no int or bool, and these take EntityView directly):
  pub fn int(e: &EntityView<'_>, field: &str) -> Option<i64>;
  pub fn boolean(e: &EntityView<'_>, field: &str) -> Option<bool>;
  pub fn number(e: &EntityView<'_>, field: &str) -> Option<f64>;   // Float or Int
  pub fn words<'a>(e: &EntityView<'a>, field: &str) -> Option<Vec<&'a str>>;  // List of Text
  ```
  `number` accepts an `Int` where a `Float` is expected, because a user typing `30` for `day_hours` means 30.0 and the UI has no reason to know the difference. `wb_constraints::{float, text, entity}` cover the rest; re-use them rather than duplicating.

- [ ] **Step 1: Write the fixture helper**

Create `crates/wb-astro/tests/common/mod.rs`:

```rust
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
```

- [ ] **Step 2: Write the failing test**

Create `crates/wb-astro/tests/kinds.rs`:

```rust
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
    assert_eq!(c.len(), before + ASTRO_KINDS.len() - 1, "planet is the only new-ish one");
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
        vec![("order", i(1)), ("class", t("rocky")), ("distance_au", f(1.0))],
        vec![("name", t("X")), ("class", t("rocky")), ("distance_au", f(1.0))],
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
            vec![("name", t("S")), ("role", t("secondary")), ("mass_solar", f(1.0))],
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
            vec![("name", t("S")), ("role", t("primary")), ("mass_solar", f(0.0))],
        )
        .is_err()
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
            vec![("index", i(1)), ("name", t("M")), ("days", i(0)), ("calendar", Value::Entity(cal))],
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
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p wb-astro --test kinds`
Expected: FAIL — `unresolved import wb_astro::register_kinds`.

- [ ] **Step 4: Write `kinds.rs`**

Create `crates/wb-astro/src/kinds.rs`:

```rust
//! The kinds this subsystem owns, their field accessors, and their type validators.
//!
//! A validator rejects data that cannot be read: text where a number belongs, a class
//! nobody defined, a month of zero days. It never rejects an implausible world — a moon
//! inside its planet's Roche limit is perfectly readable data and a bad idea, and saying
//! so is a checker's job (§6), not a validator's.

use crate::units::BodyClass;
use wb_constraints::KindCatalog;
use wb_editlog::{EditLog, EntityKind, EntityView, Value};

/// Every kind this subsystem owns.
///
/// `planet` is here because the home world carries the astro fields in §3.3 and has to be
/// visited by Constraints for the climate and spin checkers to ever fire. Registering it
/// is additive: the planet gets a verdict only when a catalog with this list is in use.
pub const ASTRO_KINDS: &[&str] = &[
    "astro.body",
    "astro.calendar",
    "astro.month",
    "astro.season",
    "astro.star",
    "planet",
];

/// The longest week a calendar may name.
const MAX_WEEKDAYS: usize = 100;

pub fn int(e: &EntityView<'_>, field: &str) -> Option<i64> {
    match e.get(field) {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    }
}

pub fn boolean(e: &EntityView<'_>, field: &str) -> Option<bool> {
    match e.get(field) {
        Some(Value::Bool(b)) => Some(*b),
        _ => None,
    }
}

/// A number field, accepting an `Int` where a `Float` is meant: a user typing `30` for
/// `day_hours` means 30.0, and the editor has no reason to know the difference.
pub fn number(e: &EntityView<'_>, field: &str) -> Option<f64> {
    match e.get(field) {
        Some(Value::Float(x)) => Some(*x),
        Some(Value::Int(n)) => Some(*n as f64),
        _ => None,
    }
}

/// A list-of-text field.
pub fn words<'a>(e: &EntityView<'a>, field: &str) -> Option<Vec<&'a str>> {
    match e.get(field) {
        Some(Value::List(items)) => items
            .iter()
            .map(|v| match v {
                Value::Text(s) => Some(s.as_str()),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

fn text<'a>(e: &EntityView<'a>, field: &str) -> Option<&'a str> {
    match e.get(field) {
        Some(Value::Text(s)) => Some(s.as_str()),
        _ => None,
    }
}

fn require_text(e: &EntityView<'_>, field: &str) -> Result<(), String> {
    match e.get(field) {
        Some(Value::Text(s)) if !s.is_empty() => Ok(()),
        _ => Err(format!("{field} is required and must be text")),
    }
}

fn optional_text(e: &EntityView<'_>, field: &str) -> Result<(), String> {
    match e.get(field) {
        None | Some(Value::Text(_)) => Ok(()),
        _ => Err(format!("{field} must be text")),
    }
}

fn optional_bool(e: &EntityView<'_>, field: &str) -> Result<(), String> {
    match e.get(field) {
        None | Some(Value::Bool(_)) => Ok(()),
        _ => Err(format!("{field} must be true or false")),
    }
}

fn optional_entity(e: &EntityView<'_>, field: &str) -> Result<(), String> {
    match e.get(field) {
        None | Some(Value::Entity(_)) => Ok(()),
        _ => Err(format!("{field} must reference an entity")),
    }
}

fn require_entity(e: &EntityView<'_>, field: &str) -> Result<(), String> {
    match e.get(field) {
        Some(Value::Entity(_)) => Ok(()),
        _ => Err(format!("{field} is required and must reference an entity")),
    }
}

/// A number field, optional, inside `[lo, hi]` when present.
fn range(e: &EntityView<'_>, field: &str, lo: f64, hi: f64) -> Result<(), String> {
    match e.get(field) {
        None => Ok(()),
        Some(Value::Float(_) | Value::Int(_)) => {
            let x = number(e, field).unwrap_or(f64::NAN);
            if x.is_finite() && (lo..=hi).contains(&x) {
                Ok(())
            } else {
                Err(format!("{field} must be a number in [{lo}, {hi}]"))
            }
        }
        _ => Err(format!("{field} must be a number")),
    }
}

/// A number field, optional, strictly positive and finite when present.
fn positive(e: &EntityView<'_>, field: &str) -> Result<(), String> {
    match e.get(field) {
        None => Ok(()),
        Some(Value::Float(_) | Value::Int(_)) => {
            let x = number(e, field).unwrap_or(f64::NAN);
            if x.is_finite() && x > 0.0 {
                Ok(())
            } else {
                Err(format!("{field} must be a positive number"))
            }
        }
        _ => Err(format!("{field} must be a number")),
    }
}

/// An integer field, optional, at least `min` when present.
fn at_least(e: &EntityView<'_>, field: &str, min: i64) -> Result<(), String> {
    match e.get(field) {
        None => Ok(()),
        Some(Value::Int(n)) if *n >= min => Ok(()),
        _ => Err(format!("{field} must be a whole number of at least {min}")),
    }
}

fn require_at_least(e: &EntityView<'_>, field: &str, min: i64) -> Result<(), String> {
    match int(e, field) {
        Some(n) if n >= min => Ok(()),
        _ => Err(format!(
            "{field} is required and must be a whole number of at least {min}"
        )),
    }
}

/// Orbit fields shared by bodies and companion stars.
fn orbit(e: &EntityView<'_>) -> Result<(), String> {
    positive(e, "period_years")?;
    positive(e, "distance_au")?;
    positive(e, "period_days")?;
    positive(e, "distance_km")?;
    range(e, "eccentricity", 0.0, 0.95)?;
    range(e, "inclination_deg", -90.0, 90.0)?;
    at_least(e, "order", 1)?;
    optional_entity(e, "parent")
}

fn validate_body(e: &EntityView<'_>) -> Result<(), String> {
    require_text(e, "name")?;
    require_at_least(e, "order", 1)?;
    let Some(class) = text(e, "class").and_then(BodyClass::parse) else {
        return Err("class must be one of rocky, water, gas_giant, ice_giant, belt, moon".into());
    };
    orbit(e)?;
    positive(e, "radius_km")?;
    positive(e, "density_kgm3")?;
    range(e, "albedo", 0.0, 1.0)?;
    optional_bool(e, "rings")?;
    match class {
        BodyClass::Belt => {
            positive(e, "inner_au")?;
            positive(e, "outer_au")?;
            let (inner, outer) = (number(e, "inner_au"), number(e, "outer_au"));
            match (inner, outer) {
                (Some(_), Some(_)) => Ok(()),
                _ => Err("a belt needs both inner_au and outer_au".into()),
            }
        }
        BodyClass::Moon => {
            if number(e, "period_days").is_none() && number(e, "distance_km").is_none() {
                return Err("a moon needs period_days or distance_km".into());
            }
            Ok(())
        }
        _ => {
            if number(e, "period_years").is_none() && number(e, "distance_au").is_none() {
                return Err("a body needs period_years or distance_au".into());
            }
            Ok(())
        }
    }
}

fn validate_star(e: &EntityView<'_>) -> Result<(), String> {
    require_text(e, "name")?;
    match text(e, "role") {
        Some("primary") | Some("companion") => {}
        _ => return Err("role must be primary or companion".into()),
    }
    match number(e, "mass_solar") {
        Some(m) if m.is_finite() && m > 0.0 => {}
        _ => return Err("mass_solar is required and must be a positive number".into()),
    }
    positive(e, "luminosity_solar")?;
    orbit(e)
}

fn validate_calendar(e: &EntityView<'_>) -> Result<(), String> {
    require_text(e, "epoch_name")?;
    match words(e, "weekdays") {
        Some(w) if !w.is_empty() && w.len() <= MAX_WEEKDAYS => {}
        _ => {
            return Err(format!(
                "weekdays is required and must be 1 to {MAX_WEEKDAYS} names"
            ));
        }
    }
    match text(e, "leap_rule") {
        None | Some("none") => Ok(()),
        Some("every_n") => match int(e, "leap_every_n") {
            Some(n) if n >= 2 => optional_entity(e, "leap_month"),
            _ => Err("leap_rule = every_n needs leap_every_n of at least 2".into()),
        },
        _ => Err("leap_rule must be none or every_n".into()),
    }
}

fn validate_month(e: &EntityView<'_>) -> Result<(), String> {
    require_text(e, "name")?;
    require_at_least(e, "index", 1)?;
    require_at_least(e, "days", 1)?;
    require_entity(e, "calendar")
}

fn validate_season(e: &EntityView<'_>) -> Result<(), String> {
    require_text(e, "name")?;
    require_entity(e, "start_month")?;
    require_entity(e, "end_month")?;
    require_at_least(e, "start_day", 1)?;
    require_at_least(e, "end_day", 1)?;
    require_entity(e, "calendar")
}

/// The home world's astronomy fields (§3.3). Every one is optional, and every field this
/// subsystem does not own is left alone — other subsystems write to the planet too.
fn validate_planet(e: &EntityView<'_>) -> Result<(), String> {
    positive(e, "year_days")?;
    positive(e, "day_hours")?;
    positive(e, "radius_km")?;
    positive(e, "gravity_ms2")?;
    range(e, "axial_tilt_deg", 0.0, 90.0)?;
    range(e, "eccentricity", 0.0, 0.95)?;
    at_least(e, "order", 1)?;
    optional_bool(e, "retrograde")?;
    optional_text(e, "name")
}

fn validate(kind: &str, e: &EntityView<'_>) -> Result<(), String> {
    match kind {
        "astro.body" => validate_body(e),
        "astro.star" => validate_star(e),
        "astro.calendar" => validate_calendar(e),
        "astro.month" => validate_month(e),
        "astro.season" => validate_season(e),
        "planet" => validate_planet(e),
        _ => Ok(()),
    }
}

/// Registers a type validator for every kind in [`ASTRO_KINDS`].
pub fn register_kinds(log: &mut EditLog) {
    for &kind in ASTRO_KINDS {
        let k = EntityKind::new(kind).expect("astro kinds are valid keys");
        log.register_validator(k, Box::new(move |e: EntityView<'_>| validate(kind, &e)));
    }
}

/// Adds [`ASTRO_KINDS`] to a Constraints kind catalog, so these entities get verdicts.
pub fn register_catalog(catalog: &mut KindCatalog) {
    for &kind in ASTRO_KINDS {
        catalog.register(kind);
    }
}
```

- [ ] **Step 5: Export it**

In `crates/wb-astro/src/lib.rs`, add `mod kinds;` and:

```rust
pub use kinds::{ASTRO_KINDS, boolean, int, number, register_catalog, register_kinds, words};
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p wb-astro --test kinds`
Expected: PASS — all 11 tests.

`the_catalog_gains_every_astro_kind` asserts `before + ASTRO_KINDS.len() - 1` because `planet` is the only one of the six whose name could plausibly already be in a starter catalog; if that assertion is off by one, print `c.kinds()` and fix the *test's* arithmetic, not the registration.

- [ ] **Step 7: Run the full gate**

Run: `cargo fmt --all && scripts/check.sh`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/wb-astro
git commit -m "$(cat <<'EOF'
feat(astro): kinds, field accessors and type validators

astro.body, astro.star, astro.calendar, astro.month, astro.season and planet,
with validators that reject unreadable data and never an implausible world.
Accessors for the Value shapes wb-constraints does not cover (int, bool,
number-accepting-int, list-of-text), plus catalog registration.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: `SkyModel::build` — parent resolution and derived values

The heart of the crate: read the entities, resolve every parent, derive every missing mass, distance and period, and refuse only the six things that make a system unreadable. After this task `SkyModel` exists and knows where everything is; the outputs land in Tasks 7–10.

**Files:**
- Create: `crates/wb-astro/src/system.rs`
- Create: `crates/wb-astro/src/calendar.rs` (the data model only; dates come in Task 9)
- Create: `crates/wb-astro/src/sky.rs`
- Modify: `crates/wb-astro/src/lib.rs`
- Modify: `crates/wb-astro/tests/common/mod.rs` (add the Earth and Aethoria fixtures)
- Test: `crates/wb-astro/tests/system.rs`

**Interfaces:**
- Consumes: `kepler::*` (Task 4), `units::*` (Task 3), `kinds::{int, number, words}` (Task 5).
- Produces:
  ```rust
  /// Everything known about one body, with every derived value resolved. SI units.
  #[derive(Clone, Debug, PartialEq)]
  pub struct BodyView {
      pub id: EntityId,
      pub name: String,
      pub class: BodyClass,
      pub order: i64,
      /// `None` for the primary star, which is the system's origin.
      pub parent: Option<EntityId>,
      pub a_m: f64,              // semi-major axis about the parent; 0 for the primary
      pub period_s: f64,         // 0 for the primary
      pub eccentricity: f64,
      pub inclination_rad: f64,
      pub radius_m: f64,
      pub density_kgm3: f64,
      pub albedo: f64,
      pub mass_kg: f64,
      pub luminosity_w: Option<f64>,     // Some only for stars
      pub rings: bool,
      pub extent_m: Option<(f64, f64)>,  // Some only for a belt
      pub stated_a_m: Option<f64>,       // what the user typed, before derivation
      pub stated_period_s: Option<f64>,
      pub is_home: bool,
      pub is_primary_star: bool,
  }
  impl BodyView {
      pub fn is_star(&self) -> bool;
      pub fn perihelion_m(&self) -> f64;  // a(1 − e)
      pub fn aphelion_m(&self) -> f64;    // a(1 + e)
  }

  // calendar.rs — data only in this task
  #[derive(Clone, Debug, PartialEq)]
  pub struct MonthView { pub id: EntityId, pub index: i64, pub name: String, pub days: i64 }
  #[derive(Clone, Debug, PartialEq)]
  pub struct SeasonView {
      pub id: EntityId, pub name: String,
      pub start_month: EntityId, pub start_day: i64,
      pub end_month: EntityId, pub end_day: i64,
  }
  #[derive(Clone, Debug, PartialEq)]
  pub struct CalendarModel {
      pub id: EntityId,
      pub epoch_name: String,
      pub weekdays: Vec<String>,
      pub leap_every_n: Option<i64>,
      pub leap_month: Option<EntityId>,
      pub months: Vec<MonthView>,     // sorted by (index, EntityId)
      pub seasons: Vec<SeasonView>,   // sorted by EntityId
  }
  impl CalendarModel {
      /// Total stated month days in a common year.
      pub fn month_days(&self) -> i64;
      pub fn month(&self, id: EntityId) -> Option<&MonthView>;
      pub(crate) fn read(state: &State) -> Result<CalendarModel, AstroError>;
  }

  // sky.rs
  pub struct SkyModel { /* private */ }
  impl SkyModel {
      pub fn build(state: &State) -> Result<SkyModel, AstroError>;
      pub fn bodies(&self) -> impl Iterator<Item = &BodyView>;
      pub fn body(&self, id: EntityId) -> Option<&BodyView>;
      pub fn home(&self) -> &BodyView;
      pub fn primary_star(&self) -> &BodyView;
      pub fn stars(&self) -> impl Iterator<Item = &BodyView>;
      pub fn moons_of_home(&self) -> impl Iterator<Item = &BodyView>;
      pub fn calendar(&self) -> &CalendarModel;
      pub fn year_days(&self) -> f64;
      pub fn day_hours(&self) -> f64;
      pub fn axial_tilt_rad(&self) -> f64;
      pub fn is_retrograde(&self) -> bool;
      /// Position relative to the primary star, metres, at day-of-year `day`.
      pub fn heliocentric(&self, id: EntityId, day: f64) -> Option<Vec3>;
      /// Position relative to the home world, metres (spec §4.4).
      pub fn position(&self, id: EntityId, day: f64) -> Option<Vec3>;
      /// Seconds since day 0.
      pub fn seconds_of(&self, day: f64) -> f64;
  }
  ```
  Defaults when the planet entity is silent: `year_days = 365.2422`, `day_hours = 24.0`,
  `axial_tilt_deg = 23.44`, `eccentricity = 0`, `retrograde = false`, `radius_km = 6371`,
  `gravity_ms2 = 9.80665`. Units in the data model are **local**: `period_years` is local
  years (`year_days` × `day_hours` hours) and `period_days` is local days, because the
  lore's calendar sets the orbits, not the reverse (spec §4.1).

- [ ] **Step 1: Extend the fixtures**

Append to `crates/wb-astro/tests/common/mod.rs`:

```rust
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
        vec![("leap_month", Value::Entity(february.expect("February exists")))],
    );
    cal
}

pub const MONTH_NAMES: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September",
    "October", "November", "December",
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
        let mut fields = vec![
            ("name", t(name)),
            ("order", i(order)),
            ("class", t(class)),
        ];
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
    "Mehr", "Solspire", "Shahrivar", "Tejas", "Azar", "Geminsol", "Yugma", "Sandhya", "Mot",
    "Rajani", "Hemanta", "Himani", "Vasanta",
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
```

- [ ] **Step 2: Write the failing test**

Create `crates/wb-astro/tests/system.rs`:

```rust
mod common;

use common::{
    add, add_aethorian_calendar, add_gregorian_calendar, aethoria, earth_system, f, i, named,
    new_log, set, t,
};
use wb_astro::{AU_M, AstroError, BodyClass, SkyModel};
use wb_editlog::{EntityId, Value};

fn close(got: f64, want: f64, rel: f64) {
    let err = (got - want).abs() / want.abs();
    assert!(err <= rel, "got {got}, want {want} (relative error {err} > {rel})");
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
        vec![("name", t("A")), ("role", t("companion")), ("mass_solar", f(1.0))],
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
        vec![("name", t("Other")), ("role", t("primary")), ("mass_solar", f(1.0))],
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
        vec![("name", t("S")), ("role", t("primary")), ("mass_solar", f(1.0))],
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
    assert_eq!(jove.stated_a_m.is_some(), true);
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
    // L = M^3.5 for M = 0.33 is about 0.0117 solar luminosities.
    close(l / wb_astro::SOLAR_LUMINOSITY_W, 0.0117, 0.05);
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
    assert_eq!(sky.position(EntityId::PLANET, 10.0), Some(wb_grid::Vec3::new(0.0, 0.0, 0.0)));
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
        vec![("name", t("S")), ("role", t("primary")), ("mass_solar", f(1.0))],
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
        vec![("name", t("S")), ("role", t("primary")), ("mass_solar", f(1.0))],
    );
    add_aethorian_calendar(&mut log);
    let sky = SkyModel::build(log.state()).unwrap();
    close(sky.year_days(), 365.2422, 1e-12);
    close(sky.day_hours(), 24.0, 1e-12);
    close(sky.home().radius_m, 6_371_000.0, 1e-12);
    close(sky.home().a_m, AU_M, 0.001);
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p wb-astro --test system`
Expected: FAIL — `unresolved imports wb_astro::SkyModel, wb_astro::BodyView`.

- [ ] **Step 4: Write `system.rs`**

Create `crates/wb-astro/src/system.rs`:

```rust
//! Reading the entities into a resolved system.
//!
//! Three passes, each a prerequisite of the next: collect the raw records, prove the
//! parent graph is a tree rooted at the primary star, then resolve derived values in
//! depth order so a parent's mass is always known before a child needs it.

use crate::error::AstroError;
use crate::kepler::{period_s, semi_major_m};
use crate::kinds::{boolean, int, number};
use crate::units::{
    AU_M, BodyClass, SECONDS_PER_HOUR, SOLAR_LUMINOSITY_W, SOLAR_MASS_KG, luminosity_from_mass_solar,
    mass_from_density, mass_from_gravity,
};
use std::collections::BTreeMap;
use wb_editlog::{EntityId, EntityView, State};

pub const DEFAULT_YEAR_DAYS: f64 = 365.2422;
pub const DEFAULT_DAY_HOURS: f64 = 24.0;
pub const DEFAULT_TILT_DEG: f64 = 23.44;

/// Everything known about one body, with every derived value resolved. SI units.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyView {
    pub id: EntityId,
    pub name: String,
    pub class: BodyClass,
    pub order: i64,
    /// `None` for the primary star, which is the system's origin.
    pub parent: Option<EntityId>,
    pub a_m: f64,
    pub period_s: f64,
    pub eccentricity: f64,
    pub inclination_rad: f64,
    pub radius_m: f64,
    pub density_kgm3: f64,
    pub albedo: f64,
    pub mass_kg: f64,
    /// Radiated power in watts; `Some` only for stars.
    pub luminosity_w: Option<f64>,
    pub rings: bool,
    /// A belt's inner and outer edge in metres; `Some` only for a belt.
    pub extent_m: Option<(f64, f64)>,
    /// What the user actually typed, before derivation — the period/distance conflict
    /// checker compares the two, so it has to know which were stated.
    pub stated_a_m: Option<f64>,
    pub stated_period_s: Option<f64>,
    pub is_home: bool,
    pub is_primary_star: bool,
}

impl BodyView {
    pub fn is_star(&self) -> bool {
        self.class == BodyClass::Star
    }

    pub fn perihelion_m(&self) -> f64 {
        self.a_m * (1.0 - self.eccentricity)
    }

    pub fn aphelion_m(&self) -> f64 {
        self.a_m * (1.0 + self.eccentricity)
    }
}

/// The home world's own settings, read once.
#[derive(Clone, Copy, Debug)]
pub struct HomeSettings {
    pub year_days: f64,
    pub day_hours: f64,
    pub axial_tilt_rad: f64,
    pub eccentricity: f64,
    pub retrograde: bool,
    pub radius_m: f64,
    pub gravity_ms2: f64,
    pub order: i64,
}

impl HomeSettings {
    pub fn read(state: &State) -> HomeSettings {
        let planet = state.entity(EntityId::PLANET);
        let get = |field: &str| planet.as_ref().and_then(|p| number(p, field));
        let positive = |field: &str, default: f64| match get(field) {
            Some(x) if x.is_finite() && x > 0.0 => x,
            _ => default,
        };
        HomeSettings {
            year_days: positive("year_days", DEFAULT_YEAR_DAYS),
            day_hours: positive("day_hours", DEFAULT_DAY_HOURS),
            axial_tilt_rad: match get("axial_tilt_deg") {
                Some(d) if d.is_finite() => d.clamp(0.0, 90.0).to_radians(),
                _ => DEFAULT_TILT_DEG.to_radians(),
            },
            eccentricity: match get("eccentricity") {
                Some(e) if e.is_finite() => e.clamp(0.0, 0.95),
                _ => 0.0,
            },
            retrograde: planet
                .as_ref()
                .and_then(|p| boolean(p, "retrograde"))
                .unwrap_or(false),
            radius_m: positive("radius_km", crate::units::EARTH_RADIUS_KM) * 1_000.0,
            gravity_ms2: positive("gravity_ms2", crate::units::EARTH_GRAVITY_MS2),
            order: planet
                .as_ref()
                .and_then(|p| int(p, "order"))
                .filter(|n| *n >= 1)
                .unwrap_or(1),
        }
    }

    /// Length of a local year in seconds.
    pub fn year_s(&self) -> f64 {
        self.year_days * self.day_hours * SECONDS_PER_HOUR
    }

    /// Length of a local day in seconds.
    pub fn day_s(&self) -> f64 {
        self.day_hours * SECONDS_PER_HOUR
    }
}

/// One entity, read but not yet resolved.
struct Raw {
    id: EntityId,
    name: String,
    class: BodyClass,
    order: i64,
    parent: Option<EntityId>,
    stated_a_m: Option<f64>,
    stated_period_s: Option<f64>,
    eccentricity: f64,
    inclination_rad: f64,
    radius_m: Option<f64>,
    density_kgm3: Option<f64>,
    albedo: Option<f64>,
    rings: bool,
    extent_m: Option<(f64, f64)>,
    mass_solar: Option<f64>,
    luminosity_solar: Option<f64>,
    is_primary_star: bool,
    is_home: bool,
}

fn clamped(v: Option<f64>, lo: f64, hi: f64, default: f64) -> f64 {
    match v {
        Some(x) if x.is_finite() => x.clamp(lo, hi),
        _ => default,
    }
}

fn read_positive(e: &EntityView<'_>, field: &str) -> Option<f64> {
    number(e, field).filter(|x| x.is_finite() && *x > 0.0)
}

fn read_body(e: &EntityView<'_>, home: &HomeSettings) -> Option<Raw> {
    let class = BodyClass::parse(match e.get("class") {
        Some(wb_editlog::Value::Text(s)) => s.as_str(),
        _ => return None,
    })?;
    let name = match e.get("name") {
        Some(wb_editlog::Value::Text(s)) => s.clone(),
        _ => return None,
    };
    let (stated_a_m, stated_period_s, extent_m) = match class {
        BodyClass::Moon => (
            read_positive(e, "distance_km").map(|km| km * 1_000.0),
            read_positive(e, "period_days").map(|d| d * home.day_s()),
            None,
        ),
        BodyClass::Belt => {
            let inner = read_positive(e, "inner_au")? * AU_M;
            let outer = read_positive(e, "outer_au")? * AU_M;
            let (lo, hi) = if inner <= outer {
                (inner, outer)
            } else {
                (outer, inner)
            };
            (
                Some((lo + hi) / 2.0),
                read_positive(e, "period_years").map(|y| y * home.year_s()),
                Some((lo, hi)),
            )
        }
        _ => (
            read_positive(e, "distance_au").map(|au| au * AU_M),
            read_positive(e, "period_years").map(|y| y * home.year_s()),
            None,
        ),
    };
    Some(Raw {
        id: e.id,
        name,
        class,
        order: int(e, "order").filter(|n| *n >= 1).unwrap_or(1),
        parent: match e.get("parent") {
            Some(wb_editlog::Value::Entity(p)) => Some(*p),
            _ => None,
        },
        stated_a_m,
        stated_period_s,
        eccentricity: clamped(number(e, "eccentricity"), 0.0, 0.95, 0.0),
        inclination_rad: clamped(number(e, "inclination_deg"), -90.0, 90.0, 0.0).to_radians(),
        radius_m: read_positive(e, "radius_km").map(|km| km * 1_000.0),
        density_kgm3: read_positive(e, "density_kgm3"),
        albedo: number(e, "albedo").filter(|a| a.is_finite()).map(|a| a.clamp(0.0, 1.0)),
        rings: boolean(e, "rings").unwrap_or(false),
        extent_m,
        mass_solar: None,
        luminosity_solar: None,
        is_primary_star: false,
        is_home: false,
    })
}

fn read_star(e: &EntityView<'_>, home: &HomeSettings) -> Option<Raw> {
    let name = match e.get("name") {
        Some(wb_editlog::Value::Text(s)) => s.clone(),
        _ => return None,
    };
    let role = match e.get("role") {
        Some(wb_editlog::Value::Text(s)) => s.clone(),
        _ => return None,
    };
    let is_primary = role == "primary";
    if !is_primary && role != "companion" {
        return None;
    }
    Some(Raw {
        id: e.id,
        name,
        class: BodyClass::Star,
        order: int(e, "order").filter(|n| *n >= 1).unwrap_or(1),
        parent: match e.get("parent") {
            Some(wb_editlog::Value::Entity(p)) => Some(*p),
            _ => None,
        },
        stated_a_m: read_positive(e, "distance_au").map(|au| au * AU_M),
        stated_period_s: read_positive(e, "period_years").map(|y| y * home.year_s()),
        eccentricity: clamped(number(e, "eccentricity"), 0.0, 0.95, 0.0),
        inclination_rad: clamped(number(e, "inclination_deg"), -90.0, 90.0, 0.0).to_radians(),
        radius_m: read_positive(e, "radius_km").map(|km| km * 1_000.0),
        density_kgm3: read_positive(e, "density_kgm3"),
        albedo: Some(0.0),
        rings: false,
        extent_m: None,
        mass_solar: read_positive(e, "mass_solar"),
        luminosity_solar: read_positive(e, "luminosity_solar"),
        is_primary_star: is_primary,
        is_home: false,
    })
}

fn home_raw(home: &HomeSettings, state: &State) -> Raw {
    let name = state
        .entity(EntityId::PLANET)
        .and_then(|p| match p.get("name") {
            Some(wb_editlog::Value::Text(s)) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "the home world".to_string());
    Raw {
        id: EntityId::PLANET,
        name,
        class: BodyClass::Home,
        order: home.order,
        parent: None, // set to the primary star during resolution
        stated_a_m: None,
        stated_period_s: Some(home.year_s()),
        eccentricity: home.eccentricity,
        inclination_rad: 0.0,
        radius_m: Some(home.radius_m),
        density_kgm3: None,
        albedo: Some(BodyClass::Home.defaults().albedo),
        rings: false,
        extent_m: None,
        mass_solar: None,
        luminosity_solar: None,
        is_primary_star: false,
        is_home: true,
    }
}

/// Reads and resolves every body. Returns the bodies in `EntityId` order, with the
/// primary star's id.
pub fn resolve(
    state: &State,
    home: &HomeSettings,
) -> Result<(Vec<BodyView>, EntityId), AstroError> {
    let mut raws: BTreeMap<EntityId, Raw> = BTreeMap::new();
    let mut primaries = Vec::new();
    for e in state.live() {
        match e.kind() {
            Some("astro.body") => {
                if let Some(r) = read_body(&e, home) {
                    raws.insert(e.id, r);
                }
            }
            Some("astro.star") => {
                if let Some(r) = read_star(&e, home) {
                    if r.is_primary_star {
                        primaries.push(e.id);
                    }
                    raws.insert(e.id, r);
                }
            }
            _ => {}
        }
    }
    let primary = match primaries.len() {
        0 => return Err(AstroError::NoPrimaryStar),
        1 => primaries[0],
        _ => return Err(AstroError::MultiplePrimaryStars),
    };
    raws.insert(EntityId::PLANET, home_raw(home, state));

    // Every node's parent defaults to the primary star; the primary itself has none.
    let ids: Vec<EntityId> = raws.keys().copied().collect();
    for id in &ids {
        let parent = raws[id].parent;
        let resolved = if *id == primary {
            None
        } else {
            match parent {
                None => Some(primary),
                Some(p) if p == *id => return Err(AstroError::ParentCycle(*id)),
                Some(p) if raws.contains_key(&p) => Some(p),
                Some(_) => return Err(AstroError::MissingParent(*id)),
            }
        };
        raws.get_mut(id).expect("id came from the map").parent = resolved;
    }

    // Depth from the primary star, which also proves there is no cycle.
    let limit = ids.len() + 1;
    let mut depth: BTreeMap<EntityId, usize> = BTreeMap::new();
    for id in &ids {
        let mut steps = 0;
        let mut cursor = *id;
        while let Some(p) = raws[&cursor].parent {
            steps += 1;
            if steps > limit {
                return Err(AstroError::ParentCycle(*id));
            }
            cursor = p;
        }
        depth.insert(*id, steps);
    }

    // Resolve shallowest first, so a parent's mass is known before its children need it.
    let mut order: Vec<EntityId> = ids.clone();
    order.sort_by_key(|id| (depth[id], *id));

    let mut out: BTreeMap<EntityId, BodyView> = BTreeMap::new();
    for id in order {
        let r = &raws[&id];
        let defaults = r.class.defaults();
        let radius_m = r.radius_m.unwrap_or(defaults.radius_km * 1_000.0);
        let density_kgm3 = r.density_kgm3.unwrap_or(defaults.density_kgm3);
        let (mass_kg, luminosity_w) = match r.class {
            BodyClass::Star => {
                let m = r.mass_solar.unwrap_or(1.0);
                let l = r.luminosity_solar.unwrap_or_else(|| luminosity_from_mass_solar(m));
                (m * SOLAR_MASS_KG, Some(l * SOLAR_LUMINOSITY_W))
            }
            BodyClass::Home => (mass_from_gravity(home.gravity_ms2, radius_m), None),
            // A belt's mass is spread over its whole annulus; nothing orbits it and it
            // perturbs nothing in a two-body model.
            BodyClass::Belt => (0.0, None),
            _ => (mass_from_density(radius_m, density_kgm3), None),
        };
        let parent_mass = r
            .parent
            .and_then(|p| out.get(&p))
            .map(|p| p.mass_kg)
            .unwrap_or(0.0);
        // Both stated: the distance drives the geometry and the clash is a verdict.
        let (a_m, period) = match (r.stated_a_m, r.stated_period_s) {
            (Some(a), _) => (a, period_s(a, parent_mass)),
            (None, Some(p)) => (semi_major_m(p, parent_mass), p),
            (None, None) => (0.0, 0.0),
        };
        let period_s_final = if r.parent.is_none() { 0.0 } else { period };
        out.insert(
            id,
            BodyView {
                id,
                name: r.name.clone(),
                class: r.class,
                order: r.order,
                parent: r.parent,
                a_m: if r.parent.is_none() { 0.0 } else { a_m },
                period_s: period_s_final,
                eccentricity: r.eccentricity,
                inclination_rad: r.inclination_rad,
                radius_m,
                density_kgm3,
                albedo: r.albedo.unwrap_or(defaults.albedo),
                mass_kg,
                luminosity_w,
                rings: r.rings,
                extent_m: r.extent_m,
                stated_a_m: r.stated_a_m,
                stated_period_s: r.stated_period_s,
                is_home: r.is_home,
                is_primary_star: r.is_primary_star,
            },
        );
    }
    Ok((out.into_values().collect(), primary))
}
```

- [ ] **Step 5: Write `calendar.rs` (data model)**

Create `crates/wb-astro/src/calendar.rs`:

```rust
//! The calendar as data. Date arithmetic lives alongside it; this is what is read from
//! the entities.

use crate::error::AstroError;
use crate::kinds::{int, number, words};
use wb_editlog::{EntityId, State, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct MonthView {
    pub id: EntityId,
    pub index: i64,
    pub name: String,
    pub days: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SeasonView {
    pub id: EntityId,
    pub name: String,
    pub start_month: EntityId,
    pub start_day: i64,
    pub end_month: EntityId,
    pub end_day: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalendarModel {
    pub id: EntityId,
    pub epoch_name: String,
    pub weekdays: Vec<String>,
    /// `Some(n)` when the leap rule is `every_n`.
    pub leap_every_n: Option<i64>,
    pub leap_month: Option<EntityId>,
    /// Months in `(index, EntityId)` order.
    pub months: Vec<MonthView>,
    /// Seasons in `EntityId` order.
    pub seasons: Vec<SeasonView>,
}

fn text(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::Text(s)) => Some(s.clone()),
        _ => None,
    }
}

fn entity(v: Option<&Value>) -> Option<EntityId> {
    match v {
        Some(Value::Entity(e)) => Some(*e),
        _ => None,
    }
}

impl CalendarModel {
    /// The stated month days of a common (non-leap) year.
    pub fn month_days(&self) -> i64 {
        self.months.iter().map(|m| m.days).sum()
    }

    pub fn month(&self, id: EntityId) -> Option<&MonthView> {
        self.months.iter().find(|m| m.id == id)
    }

    /// Position of a month in `months`, which is its place in the year.
    pub fn month_position(&self, id: EntityId) -> Option<usize> {
        self.months.iter().position(|m| m.id == id)
    }

    /// Days in one month of a given year, honouring the leap rule.
    pub fn days_in(&self, position: usize, leap: bool) -> i64 {
        match self.months.get(position) {
            None => 0,
            Some(m) => {
                let bonus = i64::from(leap && self.leap_month == Some(m.id));
                m.days + bonus
            }
        }
    }

    /// Reads the one calendar and its months and seasons.
    ///
    /// With several `astro.calendar` entities the lowest `EntityId` wins — the one made
    /// first. Months and seasons naming a different calendar are skipped.
    pub(crate) fn read(state: &State) -> Result<CalendarModel, AstroError> {
        let cal = state
            .live()
            .find(|e| e.kind() == Some("astro.calendar"))
            .ok_or(AstroError::NoCalendar)?;
        let id = cal.id;
        let mut months: Vec<MonthView> = Vec::new();
        let mut seasons: Vec<SeasonView> = Vec::new();
        for e in state.live() {
            match e.kind() {
                Some("astro.month") if entity(e.get("calendar")) == Some(id) => {
                    let (Some(index), Some(days), Some(name)) = (
                        int(&e, "index"),
                        int(&e, "days"),
                        text(e.get("name")),
                    ) else {
                        continue;
                    };
                    if days >= 1 && index >= 1 {
                        months.push(MonthView {
                            id: e.id,
                            index,
                            name,
                            days,
                        });
                    }
                }
                Some("astro.season") if entity(e.get("calendar")) == Some(id) => {
                    let (
                        Some(name),
                        Some(start_month),
                        Some(start_day),
                        Some(end_month),
                        Some(end_day),
                    ) = (
                        text(e.get("name")),
                        entity(e.get("start_month")),
                        int(&e, "start_day"),
                        entity(e.get("end_month")),
                        int(&e, "end_day"),
                    ) else {
                        continue;
                    };
                    seasons.push(SeasonView {
                        id: e.id,
                        name,
                        start_month,
                        start_day,
                        end_month,
                        end_day,
                    });
                }
                _ => {}
            }
        }
        if months.is_empty() {
            return Err(AstroError::NoMonths);
        }
        months.sort_by_key(|m| (m.index, m.id));
        seasons.sort_by_key(|s| s.id);
        let leap_every_n = match text(cal.get("leap_rule")).as_deref() {
            Some("every_n") => int(&cal, "leap_every_n").filter(|n| *n >= 2),
            _ => None,
        };
        Ok(CalendarModel {
            id,
            epoch_name: text(cal.get("epoch_name")).unwrap_or_default(),
            weekdays: words(&cal, "weekdays")
                .unwrap_or_default()
                .into_iter()
                .map(str::to_string)
                .collect(),
            leap_every_n,
            leap_month: entity(cal.get("leap_month")).filter(|m| months.iter().any(|x| x.id == *m)),
            months,
            seasons,
        })
    }
}

/// Unused here; keeps `number` imported for the date arithmetic that follows.
pub(crate) fn _number_is_used(e: &wb_editlog::EntityView<'_>) -> Option<f64> {
    number(e, "days")
}
```

Delete `_number_is_used` and the `number` import if the compiler does not need them — they exist only so this file compiles standalone; Task 9 does not need them either.

- [ ] **Step 6: Write `sky.rs`**

Create `crates/wb-astro/src/sky.rs`:

```rust
//! `SkyModel`: the resolved system, and every number the rest of the engine asks of it.

use crate::calendar::CalendarModel;
use crate::error::AstroError;
use crate::kepler::{mean_anomaly, orbit_position};
use crate::system::{BodyView, HomeSettings, resolve};
use crate::units::BodyClass;
use std::collections::BTreeMap;
use wb_editlog::{EntityId, State};
use wb_grid::Vec3;

/// A world's whole sky, derived from its entities and never stored.
pub struct SkyModel {
    bodies: Vec<BodyView>,
    index: BTreeMap<EntityId, usize>,
    primary: usize,
    home: usize,
    stars: Vec<usize>,
    moons_of_home: Vec<usize>,
    settings: HomeSettings,
    calendar: CalendarModel,
}

impl SkyModel {
    /// Reads a system from world state.
    ///
    /// Fails only on the six things in [`AstroError`] — a system with no star, two
    /// primaries, a dangling or circular parent, or no calendar. Everything else, however
    /// unstable or absurd, builds fine and is reported by the checkers in §6.
    pub fn build(state: &State) -> Result<SkyModel, AstroError> {
        let settings = HomeSettings::read(state);
        let calendar = CalendarModel::read(state)?;
        let (bodies, primary_id) = resolve(state, &settings)?;
        let index: BTreeMap<EntityId, usize> =
            bodies.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
        let primary = *index
            .get(&primary_id)
            .ok_or(AstroError::NoPrimaryStar)?;
        let home = *index
            .get(&EntityId::PLANET)
            .expect("the home world is always resolved");
        let stars = bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.is_star())
            .map(|(i, _)| i)
            .collect();
        let moons_of_home = bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.class == BodyClass::Moon && b.parent == Some(EntityId::PLANET))
            .map(|(i, _)| i)
            .collect();
        Ok(SkyModel {
            bodies,
            index,
            primary,
            home,
            stars,
            moons_of_home,
            settings,
            calendar,
        })
    }

    /// Every body, star and the home world, in `EntityId` order.
    pub fn bodies(&self) -> impl Iterator<Item = &BodyView> {
        self.bodies.iter()
    }

    pub fn body(&self, id: EntityId) -> Option<&BodyView> {
        self.index.get(&id).map(|i| &self.bodies[*i])
    }

    pub fn home(&self) -> &BodyView {
        &self.bodies[self.home]
    }

    pub fn primary_star(&self) -> &BodyView {
        &self.bodies[self.primary]
    }

    pub fn stars(&self) -> impl Iterator<Item = &BodyView> {
        self.stars.iter().map(|i| &self.bodies[*i])
    }

    pub fn moons_of_home(&self) -> impl Iterator<Item = &BodyView> {
        self.moons_of_home.iter().map(|i| &self.bodies[*i])
    }

    pub fn calendar(&self) -> &CalendarModel {
        &self.calendar
    }

    pub fn year_days(&self) -> f64 {
        self.settings.year_days
    }

    pub fn day_hours(&self) -> f64 {
        self.settings.day_hours
    }

    pub fn axial_tilt_rad(&self) -> f64 {
        self.settings.axial_tilt_rad
    }

    pub fn is_retrograde(&self) -> bool {
        self.settings.retrograde
    }

    pub(crate) fn settings(&self) -> &HomeSettings {
        &self.settings
    }

    /// Seconds since day 0 — the epoch at which every body is at mean anomaly 0.
    pub fn seconds_of(&self, day: f64) -> f64 {
        day * self.settings.day_s()
    }

    /// Position relative to the primary star, in metres.
    pub fn heliocentric(&self, id: EntityId, day: f64) -> Option<Vec3> {
        let mut i = *self.index.get(&id)?;
        let t = self.seconds_of(day);
        let mut p = Vec3::new(0.0, 0.0, 0.0);
        // Walk up the parent chain, which `resolve` proved is finite.
        for _ in 0..=self.bodies.len() {
            let b = &self.bodies[i];
            let Some(parent) = b.parent else { return Some(p) };
            p = p.plus(orbit_position(
                b.a_m,
                b.eccentricity,
                b.inclination_rad,
                mean_anomaly(t, b.period_s),
            ));
            i = *self.index.get(&parent)?;
        }
        Some(p)
    }

    /// Position relative to the home world, in metres (spec §4.4).
    pub fn position(&self, id: EntityId, day: f64) -> Option<Vec3> {
        let there = self.heliocentric(id, day)?;
        let here = self.heliocentric(EntityId::PLANET, day)?;
        Some(Vec3::new(
            there.x - here.x,
            there.y - here.y,
            there.z - here.z,
        ))
    }
}
```

- [ ] **Step 7: Export it**

In `crates/wb-astro/src/lib.rs`, add the modules (alphabetical) and re-exports:

```rust
mod calendar;
mod sky;
mod system;

pub use calendar::{CalendarModel, MonthView, SeasonView};
pub use sky::SkyModel;
pub use system::{BodyView, DEFAULT_DAY_HOURS, DEFAULT_TILT_DEG, DEFAULT_YEAR_DAYS, HomeSettings};
```

- [ ] **Step 8: Run the tests**

Run: `cargo test -p wb-astro --test system`
Expected: PASS — all 16 tests.

`the_home_world_sits_where_its_year_puts_it` is the one to watch: if `a_m` is off by more than 0.1% from an AU, the local-year conversion in `HomeSettings::year_s` is wrong, not the Kepler solver (Task 4's tests already pin that down).

- [ ] **Step 9: Run the full gate**

Run: `cargo fmt --all && scripts/check.sh`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/wb-astro
git commit -m "$(cat <<'EOF'
feat(astro): SkyModel::build with parent resolution and derived values

Reads stars, bodies, the home world and the calendar out of world state;
proves the parent graph is a tree rooted at the primary star; derives masses
from gravity, density or solar mass, and the missing one of period/distance
from Kepler's third law in depth order. Positions are heliocentric or relative
to the home world. Only the six AstroError cases refuse to build.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: Insolation and daylight

Sunlight at a latitude on a day, summed over every star, and the length of the day from the primary. This is the output the climate stage will consume, so it has to match published figures for Earth before anyone trusts it for a two-sun world.

**Files:**
- Modify: `crates/wb-astro/src/sky.rs` (add the two methods and their helpers)
- Modify: `crates/wb-astro/src/lib.rs` (export the helpers used by tests)
- Test: `crates/wb-astro/tests/sky.rs`

**Interfaces:**
- Consumes: `SkyModel` (Task 6), `kepler::ecliptic_longitude` (Task 4).
- Produces:
  ```rust
  impl SkyModel {
      /// Daily-mean flux at the top of the atmosphere, W·m⁻², summed over every star.
      pub fn insolation(&self, lat: LatLon, day: f64) -> f64;
      /// Hours of daylight from the primary star, in local hours.
      pub fn daylight_hours(&self, lat: LatLon, day: f64) -> f64;
      /// Declination of the sub-stellar point for a direction to a star, radians.
      pub fn declination(&self, to_star: Vec3) -> f64;
  }
  /// Half-day hour angle: `acos(−tan φ · tan δ)`, clamped, so a polar day gives π and a
  /// polar night gives 0.
  pub fn hour_angle(lat_rad: f64, declination_rad: f64) -> f64;
  /// Daily-mean insolation as a fraction of the flux at the sub-stellar point.
  pub fn daily_mean_factor(lat_rad: f64, declination_rad: f64) -> f64;
  ```
  `lat.lon` is ignored: a daily mean is the same all the way round a parallel. Stars are summed in `BodyView` order, which is `EntityId` order — a fixed-order reduction, as the determinism rules require.

- [ ] **Step 1: Write the failing test**

Create `crates/wb-astro/tests/sky.rs`:

```rust
mod common;

use common::{add, aethoria, earth_system, f, i, named, set, t};
use std::f64::consts::PI;
use wb_astro::{SkyModel, daily_mean_factor, hour_angle};
use wb_editlog::{EntityId, Value};
use wb_grid::LatLon;

fn close(got: f64, want: f64, rel: f64) {
    let err = (got - want).abs() / want.abs();
    assert!(err <= rel, "got {got}, want {want} (relative error {err} > {rel})");
}

/// Annual mean of `f` over a year, by whole days.
fn annual_mean(days: f64, f: impl Fn(f64) -> f64) -> f64 {
    let n = days.round() as i64;
    let mut sum = 0.0;
    for d in 0..n {
        sum += f(d as f64);
    }
    sum / n as f64
}

#[test]
fn the_hour_angle_covers_equinox_polar_day_and_polar_night() {
    // Equinox anywhere: half a turn of daylight.
    close(hour_angle(0.5, 0.0), PI / 2.0, 1e-12);
    close(hour_angle(0.0, 0.0), PI / 2.0, 1e-12);
    // Northern summer at the north pole: all day.
    close(hour_angle(PI / 2.0, 0.4), PI, 1e-9);
    // And no day at all at the south pole.
    assert_eq!(hour_angle(-PI / 2.0, 0.4), 0.0);
    for (lat, decl) in [(0.0, 0.0), (1.0, 0.3), (-1.2, -0.2), (PI / 2.0, 0.0)] {
        let h = hour_angle(lat, decl);
        assert!((0.0..=PI).contains(&h), "hour_angle({lat}, {decl}) = {h}");
    }
}

#[test]
fn the_daily_mean_factor_is_never_negative() {
    for i in 0..40 {
        let lat = -PI / 2.0 + PI * i as f64 / 39.0;
        for j in 0..20 {
            let decl = -0.41 + 0.82 * j as f64 / 19.0;
            let k = daily_mean_factor(lat, decl);
            assert!(k >= 0.0 && k <= 1.0 && k.is_finite(), "k({lat}, {decl}) = {k}");
        }
    }
    // At the sub-stellar point of an equinox equator, a quarter of the sub-stellar flux.
    close(daily_mean_factor(0.0, 0.0), 1.0 / PI, 1e-12);
}

#[test]
fn earths_annual_mean_insolation_matches_the_published_figures() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let equator = annual_mean(365.2422, |d| sky.insolation(LatLon::from_degrees(0.0, 0.0), d));
    let pole = annual_mean(365.2422, |d| sky.insolation(LatLon::from_degrees(90.0, 0.0), d));
    // Top-of-atmosphere annual means: ~417 W/m² at the equator, ~173 at the pole.
    close(equator, 417.0, 0.02);
    close(pole, 173.0, 0.02);
    // The global pattern: the equator gets well over twice the pole.
    assert!(equator > 2.0 * pole);
}

#[test]
fn insolation_follows_the_seasons() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let north = LatLon::from_degrees(60.0, 0.0);
    let south = LatLon::from_degrees(-60.0, 0.0);
    // Day 0 is periapsis with the reference direction at ecliptic longitude 0, so the
    // declination is zero: an equinox. A quarter year on, one hemisphere is in summer.
    let quarter = 365.2422 / 4.0;
    let (n0, s0) = (sky.insolation(north, 0.0), sky.insolation(south, 0.0));
    close(n0, s0, 0.02);
    let (n1, s1) = (sky.insolation(north, quarter), sky.insolation(south, quarter));
    assert!(
        (n1 > s1 * 2.0) || (s1 > n1 * 2.0),
        "a solstice should be lopsided: {n1} vs {s1}"
    );
    // Half a year after that solstice, the lopsidedness reverses.
    let n3 = sky.insolation(north, 3.0 * quarter);
    let s3 = sky.insolation(south, 3.0 * quarter);
    assert!((n1 - s1).signum() != (n3 - s3).signum(), "seasons must reverse");
}

#[test]
fn insolation_is_hemisphere_symmetric_without_a_tilt() {
    let mut log = earth_system();
    set(
        &mut log,
        EntityId::PLANET,
        vec![("axial_tilt_deg", f(0.0)), ("eccentricity", f(0.0))],
    );
    let sky = SkyModel::build(log.state()).unwrap();
    for d in [0.0, 37.0, 180.5, 300.0] {
        for lat in [5.0, 23.0, 47.0, 80.0] {
            let n = sky.insolation(LatLon::from_degrees(lat, 0.0), d);
            let s = sky.insolation(LatLon::from_degrees(-lat, 40.0), d);
            assert!(
                (n - s).abs() < 1e-9,
                "lat ±{lat} on day {d}: {n} vs {s} — longitude must not matter either"
            );
        }
    }
}

#[test]
fn daylight_is_twelve_hours_at_an_equinox_and_polar_at_a_solstice() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    close(sky.daylight_hours(LatLon::from_degrees(0.0, 0.0), 0.0), 12.0, 1e-6);
    close(sky.daylight_hours(LatLon::from_degrees(51.5, 0.0), 0.0), 12.0, 1e-6);
    let quarter = 365.2422 / 4.0;
    let summer = sky.daylight_hours(LatLon::from_degrees(80.0, 0.0), quarter);
    let winter = sky.daylight_hours(LatLon::from_degrees(-80.0, 0.0), quarter);
    assert!(
        (summer == 24.0 && winter == 0.0) || (summer == 0.0 && winter == 24.0),
        "one pole should be in permanent day and the other in permanent night: {summer} / {winter}"
    );
    // London's longest day is between 16 and 17 hours.
    let longest = (0..366)
        .map(|d| sky.daylight_hours(LatLon::from_degrees(51.5, 0.0), d as f64))
        .fold(0.0_f64, f64::max);
    assert!((16.0..17.0).contains(&longest), "longest London day = {longest}");
}

#[test]
fn a_thirty_hour_day_is_measured_in_local_hours() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    close(
        sky.daylight_hours(LatLon::from_degrees(0.0, 0.0), 0.0),
        15.0,
        1e-6,
    );
    // The lore: 18 hours of daylight at peak summer, 12 at winter — so the mid-latitude
    // swing has to reach roughly that spread somewhere in the year.
    let lat = LatLon::from_degrees(40.0, 0.0);
    let hours: Vec<f64> = (0..390).map(|d| sky.daylight_hours(lat, d as f64)).collect();
    let longest = hours.iter().copied().fold(0.0_f64, f64::max);
    let shortest = hours.iter().copied().fold(30.0_f64, f64::min);
    assert!(longest > 17.0 && longest < 20.0, "longest = {longest}");
    assert!(shortest > 10.0 && shortest < 13.0, "shortest = {shortest}");
}

#[test]
fn a_second_sun_adds_its_own_light() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let lat = LatLon::from_degrees(0.0, 0.0);
    let with_both: Vec<f64> = (0..390).map(|d| sky.insolation(lat, d as f64)).collect();

    // The same system with the companion deleted.
    let mut alone = aethoria();
    let secundus = named(&alone, "Sol Secundus");
    let mut tx = alone.transact("delete");
    tx.delete(secundus);
    tx.commit().unwrap();
    let one = SkyModel::build(alone.state()).unwrap();
    let with_one: Vec<f64> = (0..390).map(|d| one.insolation(lat, d as f64)).collect();

    for (a, b) in with_both.iter().zip(&with_one) {
        assert!(a >= b, "two suns can never be dimmer than one: {a} < {b}");
    }
    let extra: f64 = with_both.iter().sum::<f64>() - with_one.iter().sum::<f64>();
    assert!(extra > 0.0, "the companion contributed nothing");
}

#[test]
fn insolation_is_finite_and_non_negative_everywhere() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    for i in 0..37 {
        let lat = LatLon::from_degrees(-90.0 + 5.0 * i as f64, 17.0);
        for d in [0.0, 1.5, 99.9, 389.0, 390.0, 1_000.0, -5.0] {
            let w = sky.insolation(lat, d);
            assert!(w.is_finite() && w >= 0.0, "insolation({lat:?}, {d}) = {w}");
            let h = sky.daylight_hours(lat, d);
            assert!(h.is_finite() && (0.0..=30.0).contains(&h), "daylight = {h}");
        }
    }
}

#[test]
fn a_starless_sky_is_dark_rather_than_a_panic() {
    // A primary star with zero luminosity: physically silly, perfectly readable.
    let mut log = common::new_log();
    add(
        &mut log,
        "astro.star",
        vec![
            ("name", t("Ember")),
            ("role", t("primary")),
            ("mass_solar", f(1.0)),
            ("luminosity_solar", f(1e-300)),
        ],
    );
    common::add_aethorian_calendar(&mut log);
    let sky = SkyModel::build(log.state()).unwrap();
    let w = sky.insolation(LatLon::from_degrees(10.0, 0.0), 5.0);
    assert!(w.is_finite() && w >= 0.0, "{w}");
    assert!(sky.daylight_hours(LatLon::from_degrees(10.0, 0.0), 5.0).is_finite());
    assert!(i(1) == Value::Int(1));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p wb-astro --test sky`
Expected: FAIL — `unresolved imports wb_astro::daily_mean_factor, wb_astro::hour_angle`.

- [ ] **Step 3: Add the geometry helpers**

Add to `crates/wb-astro/src/sky.rs`, above `impl SkyModel`:

```rust
/// Half the arc of daylight, in radians: `acos(−tan φ · tan δ)`.
///
/// Clamping the cosine is what makes polar day (π) and polar night (0) fall out of the
/// same expression instead of needing their own branches.
pub fn hour_angle(lat_rad: f64, declination_rad: f64) -> f64 {
    if !lat_rad.is_finite() || !declination_rad.is_finite() {
        return 0.0;
    }
    let c = -libm::tan(lat_rad) * libm::tan(declination_rad);
    libm::acos(c.clamp(-1.0, 1.0))
}

/// Daily-mean insolation as a fraction of the flux at the sub-stellar point.
///
/// `(1/π)·(H₀·sin φ·sin δ + cos φ·cos δ·sin H₀)` — the standard obliquity geometry,
/// integrated over one rotation.
pub fn daily_mean_factor(lat_rad: f64, declination_rad: f64) -> f64 {
    let h = hour_angle(lat_rad, declination_rad);
    let k = (h * libm::sin(lat_rad) * libm::sin(declination_rad)
        + libm::cos(lat_rad) * libm::cos(declination_rad) * libm::sin(h))
        / PI;
    if k.is_finite() { k.max(0.0) } else { 0.0 }
}
```

and add `use std::f64::consts::PI;` plus `use crate::kepler::ecliptic_longitude;` and `use wb_grid::LatLon;` to the file's imports.

- [ ] **Step 4: Add the two methods**

Add to `impl SkyModel` in `crates/wb-astro/src/sky.rs`:

```rust
    /// Declination of the sub-stellar point, radians, for a direction from the home world
    /// to a star.
    ///
    /// `δ = asin(sin ε · sin λ)`, where λ is the star's ecliptic longitude as seen from
    /// the home world. Using the star's own direction rather than the home world's orbital
    /// phase is what makes a second sun's seasons fall out of its orbit.
    pub fn declination(&self, to_star: Vec3) -> f64 {
        let lon = ecliptic_longitude(to_star);
        let s = libm::sin(self.settings.axial_tilt_rad) * libm::sin(lon);
        libm::asin(s.clamp(-1.0, 1.0))
    }

    /// Daily-mean flux at the top of the atmosphere, W·m⁻², summed over every star.
    ///
    /// Stars are summed in `EntityId` order, so the result does not depend on how the
    /// system was assembled. `lat.lon` is ignored: a daily mean is the same all the way
    /// round a parallel.
    pub fn insolation(&self, lat: LatLon, day: f64) -> f64 {
        let mut total = 0.0;
        for i in &self.stars {
            let star = &self.bodies[*i];
            let Some(luminosity) = star.luminosity_w else {
                continue;
            };
            let Some(to_star) = self.position(star.id, day) else {
                continue;
            };
            let distance = to_star.length();
            if !(distance > 0.0) || !distance.is_finite() || !luminosity.is_finite() {
                continue;
            }
            let flux = luminosity / (4.0 * PI * distance * distance);
            let k = daily_mean_factor(lat.lat, self.declination(to_star));
            let contribution = flux * k;
            if contribution.is_finite() {
                total += contribution;
            }
        }
        total
    }

    /// Hours of daylight from the primary star, in local hours.
    ///
    /// A companion adds light but not a day: the clock follows the primary (spec §4.2).
    pub fn daylight_hours(&self, lat: LatLon, day: f64) -> f64 {
        let primary = self.primary_star();
        let Some(to_star) = self.position(primary.id, day) else {
            return 0.0;
        };
        let h = hour_angle(lat.lat, self.declination(to_star));
        self.settings.day_hours * h / PI
    }
```

- [ ] **Step 5: Export the helpers**

In `crates/wb-astro/src/lib.rs`, extend the `sky` re-export:

```rust
pub use sky::{SkyModel, daily_mean_factor, hour_angle};
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p wb-astro --test sky`
Expected: PASS — all 10 tests.

If `earths_annual_mean_insolation_matches_the_published_figures` is high by about 1.7% at the equator, that is the expected residue of modelling Earth's orbit as a circle; the 2% tolerance covers it. If it is out by more than that, check `declination` before touching the tolerance — a declination driven by the home world's phase instead of the star's direction is off by half a turn and fails this test.

- [ ] **Step 7: Run the full gate**

Run: `cargo fmt --all && scripts/check.sh`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/wb-astro
git commit -m "$(cat <<'EOF'
feat(astro): insolation and daylight

Daily-mean top-of-atmosphere flux per latitude per day, summed over every
star in EntityId order, and daylight length from the primary in local hours.
Polar day and polar night fall out of the clamped hour angle. Earth's annual
means land within 2% of the published 417 and 173 W/m².

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 8: Tides, phases and brightness

Three outputs that make the sky legible: how far the water moves, how full each moon looks, and which body is the brightest thing in the sky. Several moons beating against each other is the one the lore cares about most.

**Files:**
- Modify: `crates/wb-astro/src/sky.rs`
- Modify: `crates/wb-astro/src/lib.rs`
- Test: `crates/wb-astro/tests/tides.rs`

**Interfaces:**
- Consumes: `SkyModel` with positions (Task 6), `ecliptic_longitude` (Task 4).
- Produces:
  ```rust
  impl SkyModel {
      /// Equilibrium tide range in metres on `day`, from every moon of the home world and
      /// every star.
      pub fn tide_range_m(&self, day: f64) -> f64;
      /// Illuminated fraction in `0..=1`, as seen from the home world. Stars are 1.
      pub fn phase(&self, id: EntityId, day: f64) -> Option<f64>;
      /// Magnitude-like brightness: smaller is brighter, `None` when nothing arrives.
      pub fn apparent_brightness(&self, id: EntityId, day: f64) -> Option<f64>;
  }
  /// Equilibrium tide amplitude, metres: `1.5·G·M·R²/(g·d³)`.
  pub fn tidal_amplitude_m(perturber_kg: f64, distance_m: f64, radius_m: f64, gravity_ms2: f64) -> f64;
  ```
  Tides combine as a degree-2 harmonic: each perturber contributes amplitude `A` at phase
  `2λ`, where λ is its ecliptic longitude from the home world, and the range is
  `2·|Σ A·e^{i2λ}|`. Doubling the angle is what makes two perturbers 90° apart cancel and
  two in line add — spring and neap tides, with no special cases. Perturbers are summed
  moons-then-stars, each group in `EntityId` order.

- [ ] **Step 1: Write the failing test**

Create `crates/wb-astro/tests/tides.rs`:

```rust
mod common;

use common::{aethoria, earth_system, named};
use wb_astro::{SkyModel, tidal_amplitude_m};
use wb_editlog::EntityId;

fn close(got: f64, want: f64, rel: f64) {
    let err = (got - want).abs() / want.abs();
    assert!(err <= rel, "got {got}, want {want} (relative error {err} > {rel})");
}

#[test]
fn the_moons_equilibrium_amplitude_is_about_half_a_metre() {
    let a = tidal_amplitude_m(7.342e22, 3.844e8, 6.371e6, 9.80665);
    close(a, 0.54, 0.05);
    // The Sun's is a bit under half the Moon's — the textbook ratio.
    let s = tidal_amplitude_m(1.98841e30, 1.495978707e11, 6.371e6, 9.80665);
    close(s / a, 0.46, 0.1);
}

#[test]
fn an_impossible_distance_gives_zero_rather_than_infinity() {
    assert_eq!(tidal_amplitude_m(7.3e22, 0.0, 6.4e6, 9.8), 0.0);
    assert_eq!(tidal_amplitude_m(7.3e22, -1.0, 6.4e6, 9.8), 0.0);
    assert_eq!(tidal_amplitude_m(7.3e22, 3.8e8, 6.4e6, 0.0), 0.0);
    assert!(tidal_amplitude_m(f64::NAN, 3.8e8, 6.4e6, 9.8).is_finite());
}

#[test]
fn earths_spring_tides_are_about_twice_its_neaps() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let ranges: Vec<f64> = (0..60).map(|d| sky.tide_range_m(d as f64)).collect();
    let spring = ranges.iter().copied().fold(0.0_f64, f64::max);
    let neap = ranges.iter().copied().fold(f64::MAX, f64::min);
    // Equilibrium theory, not a tide table: ~1.5 m springs, ~0.6 m neaps, so the ratio
    // sits near 2.6 where the observed ratio is about 2.2 (real oceans resonate).
    assert!((1.0..2.5).contains(&spring), "spring range = {spring}");
    assert!((0.2..1.0).contains(&neap), "neap range = {neap}");
    let ratio = spring / neap;
    assert!((2.0..3.2).contains(&ratio), "spring/neap = {ratio}");
}

#[test]
fn three_moons_beat_into_a_repeatable_maximum() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    // Cycles of 10, 15 and 30 local days: everything realigns every 30 days.
    let day_of_max = |from: i64| {
        (from..from + 30)
            .map(|d| (sky.tide_range_m(d as f64), d))
            .fold((f64::MIN, from), |best, x| if x.0 > best.0 { x } else { best })
            .1
    };
    let first = day_of_max(0);
    assert_eq!(
        (first - day_of_max(30)).rem_euclid(30),
        0,
        "the spring tide should land on the same day of the 30-day beat"
    );
    assert_eq!((first - day_of_max(300)).rem_euclid(30), 0);

    let max = sky.tide_range_m(first as f64);
    let min = (0..30)
        .map(|d| sky.tide_range_m(d as f64))
        .fold(f64::MAX, f64::min);
    assert!(max > min * 1.5, "three moons should beat: {max} vs {min}");
}

#[test]
fn tides_are_finite_and_non_negative_everywhere() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    for d in [-100.0, 0.0, 0.5, 123.4, 390.0, 10_000.0] {
        let r = sky.tide_range_m(d);
        assert!(r.is_finite() && r >= 0.0, "tide_range_m({d}) = {r}");
    }
}

#[test]
fn a_world_with_no_moons_still_has_solar_tides() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let with_moon = sky.tide_range_m(0.0);
    let mut log = earth_system();
    let luna = named(&log, "Luna");
    let mut tx = log.transact("delete");
    tx.delete(luna);
    tx.commit().unwrap();
    let solar_only = SkyModel::build(log.state()).unwrap().tide_range_m(0.0);
    assert!(solar_only > 0.0, "the star alone raises a tide");
    assert!(solar_only < with_moon, "{solar_only} should be under {with_moon}");
}

#[test]
fn a_moon_runs_through_its_whole_phase_cycle() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let luna = sky.moons_of_home().next().unwrap().id;
    let phases: Vec<f64> = (0..28).map(|d| sky.phase(luna, d as f64).unwrap()).collect();
    for p in &phases {
        assert!((0.0..=1.0).contains(p), "phase {p} outside 0..1");
    }
    let full = phases.iter().copied().fold(0.0_f64, f64::max);
    let new = phases.iter().copied().fold(1.0_f64, f64::min);
    assert!(full > 0.97, "a lunar month should reach full: {full}");
    assert!(new < 0.03, "a lunar month should reach new: {new}");
}

#[test]
fn stars_are_always_full_and_unknown_bodies_have_no_phase() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    for s in sky.stars() {
        assert_eq!(sky.phase(s.id, 17.0), Some(1.0));
    }
    assert_eq!(sky.phase(EntityId::PLANET, 3.0), None, "the home world sees itself");
    let ghost = EntityId {
        op: wb_editlog::OpId {
            lamport: 77,
            actor: wb_editlog::ActorId([2; 16]),
        },
        n: 1,
    };
    assert_eq!(sky.phase(ghost, 0.0), None);
    assert_eq!(sky.apparent_brightness(ghost, 0.0), None);
}

#[test]
fn the_primary_star_is_the_brightest_thing_in_the_sky() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let sun = sky
        .apparent_brightness(sky.primary_star().id, 0.0)
        .expect("the sun is visible");
    // Magnitude-like: smaller is brighter, and the primary is near −26 as from Earth.
    assert!(sun < -20.0, "the primary's magnitude = {sun}");
    for b in sky.bodies() {
        if b.is_primary_star || b.is_home {
            continue;
        }
        if let Some(m) = sky.apparent_brightness(b.id, 0.0) {
            assert!(m > sun, "{} at {m} outshines the primary at {sun}", b.name);
        }
    }
}

#[test]
fn luminar_outshines_the_outer_worlds() {
    // The lore calls Luminar the bright morning star; being near and reflective, it should
    // beat the distant giants from this engine's numbers alone.
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let brightest_day = |name: &str| {
        let id = named(&log, name);
        (0..390)
            .filter_map(|d| sky.apparent_brightness(id, d as f64))
            .fold(f64::MAX, f64::min)
    };
    let luminar = brightest_day("Luminar");
    for far in ["Frostweaver", "Rimefrost", "Voidwalker", "Shadowkeep"] {
        assert!(
            luminar < brightest_day(far),
            "Luminar at {luminar} should outshine {far} at {}",
            brightest_day(far)
        );
    }
}

#[test]
fn brightness_dims_with_phase() {
    let sky = SkyModel::build(earth_system().state()).unwrap();
    let luna = sky.moons_of_home().next().unwrap().id;
    let mut brightest = (f64::MAX, 0);
    let mut dimmest = (f64::MIN, 0);
    for d in 0..28 {
        if let Some(m) = sky.apparent_brightness(luna, d as f64) {
            if m < brightest.0 {
                brightest = (m, d);
            }
            if m > dimmest.0 {
                dimmest = (m, d);
            }
        }
    }
    assert!(
        dimmest.0 - brightest.0 > 1.0,
        "a full moon should be more than a magnitude brighter than a thin one: {brightest:?} / {dimmest:?}"
    );
    let full_phase = sky.phase(luna, brightest.1 as f64).unwrap();
    assert!(full_phase > 0.8, "the brightest night should be near full: {full_phase}");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p wb-astro --test tides`
Expected: FAIL — `unresolved import wb_astro::tidal_amplitude_m`.

- [ ] **Step 3: Add the tide helper**

Add to `crates/wb-astro/src/sky.rs`, beside `hour_angle`:

```rust
/// Equilibrium tide amplitude in metres: `1.5·G·M·R²/(g·d³)`.
///
/// The classical equilibrium tide — a static ocean under a static potential. It gives
/// Earth's Moon about 0.54 m, which is the right number for the wrong ocean: real tides
/// are amplified by basin resonance by a factor of two or more. For a world-builder's
/// "how big are the tides here" it is the honest model, and the one the checkers quote.
pub fn tidal_amplitude_m(
    perturber_kg: f64,
    distance_m: f64,
    radius_m: f64,
    gravity_ms2: f64,
) -> f64 {
    if !(distance_m > 0.0) || !(gravity_ms2 > 0.0) || !perturber_kg.is_finite() {
        return 0.0;
    }
    let d3 = distance_m * distance_m * distance_m;
    let a = 1.5 * G * perturber_kg * radius_m * radius_m / (gravity_ms2 * d3);
    if a.is_finite() { a.max(0.0) } else { 0.0 }
}
```

and add `use crate::units::G;` to the file's imports.

- [ ] **Step 4: Add the three methods**

Add to `impl SkyModel`:

```rust
    /// Equilibrium tide range in metres on `day`.
    ///
    /// Every moon of the home world and every star contributes a degree-2 harmonic of
    /// amplitude `A` at phase `2λ`, λ being its ecliptic longitude from the home world.
    /// Summing those phasors is what makes perturbers in line add and perturbers a
    /// quarter-turn apart cancel — spring and neap tides, with no special case for
    /// either. Moons are summed before stars, each group in `EntityId` order.
    pub fn tide_range_m(&self, day: f64) -> f64 {
        let home = self.home();
        let (radius_m, gravity) = (home.radius_m, self.settings.gravity_ms2);
        let mut sx = 0.0;
        let mut sy = 0.0;
        let perturbers = self.moons_of_home.iter().chain(self.stars.iter());
        for i in perturbers {
            let b = &self.bodies[*i];
            let Some(to_body) = self.position(b.id, day) else {
                continue;
            };
            let distance = to_body.length();
            let amplitude = tidal_amplitude_m(b.mass_kg, distance, radius_m, gravity);
            if amplitude == 0.0 {
                continue;
            }
            let two_lambda = 2.0 * ecliptic_longitude(to_body);
            sx += amplitude * libm::cos(two_lambda);
            sy += amplitude * libm::sin(two_lambda);
        }
        let range = 2.0 * (sx * sx + sy * sy).sqrt();
        if range.is_finite() { range } else { 0.0 }
    }

    /// Illuminated fraction as seen from the home world, `0..=1`.
    ///
    /// `(1 + cos α)/2` for the phase angle α at the body between the primary star and the
    /// home world. Stars are always 1; the home world has no phase of its own.
    pub fn phase(&self, id: EntityId, day: f64) -> Option<f64> {
        let body = self.body(id)?;
        if body.is_star() {
            return Some(1.0);
        }
        if body.is_home {
            return None;
        }
        let body_pos = self.heliocentric(id, day)?;
        let home_pos = self.heliocentric(EntityId::PLANET, day)?;
        let star_pos = self.heliocentric(self.primary_star().id, day)?;
        let to_home = Vec3::new(
            home_pos.x - body_pos.x,
            home_pos.y - body_pos.y,
            home_pos.z - body_pos.z,
        );
        let to_star = Vec3::new(
            star_pos.x - body_pos.x,
            star_pos.y - body_pos.y,
            star_pos.z - body_pos.z,
        );
        let (lh, ls) = (to_home.length(), to_star.length());
        if !(lh > 0.0) || !(ls > 0.0) {
            return Some(1.0);
        }
        let cos_alpha = (to_home.dot(to_star) / (lh * ls)).clamp(-1.0, 1.0);
        Some(((1.0 + cos_alpha) / 2.0).clamp(0.0, 1.0))
    }

    /// Magnitude-like brightness as seen from the home world: smaller is brighter.
    ///
    /// `−2.5·log₁₀(f / 1361 W·m⁻²)`, so the reference point is Earth's solar constant and
    /// the primary of a sun-like system comes out near −26, as it does from Earth. A star
    /// radiates; everything else reflects its primary's light over its disc, dimmed by
    /// phase. `None` when no light arrives at all.
    pub fn apparent_brightness(&self, id: EntityId, day: f64) -> Option<f64> {
        let body = self.body(id)?;
        if body.is_home {
            return None;
        }
        let to_body = self.position(id, day)?;
        let distance = to_body.length();
        if !(distance > 0.0) {
            return None;
        }
        let flux = if let Some(luminosity) = body.luminosity_w {
            luminosity / (4.0 * PI * distance * distance)
        } else {
            let star = self.primary_star();
            let star_luminosity = star.luminosity_w?;
            let from_star = self.heliocentric(id, day)?;
            let star_at = self.heliocentric(star.id, day)?;
            let sun_distance = Vec3::new(
                from_star.x - star_at.x,
                from_star.y - star_at.y,
                from_star.z - star_at.z,
            )
            .length();
            if !(sun_distance > 0.0) {
                return None;
            }
            let incident = star_luminosity / (4.0 * PI * sun_distance * sun_distance);
            let disc = body.radius_m / (2.0 * distance);
            let phase = self.phase(id, day).unwrap_or(1.0);
            incident * body.albedo * disc * disc * 2.0 * phase
        };
        if !(flux > 0.0) || !flux.is_finite() {
            return None;
        }
        let m = -2.5 * libm::log10(flux / SOLAR_CONSTANT_W);
        if m.is_finite() { Some(m) } else { None }
    }
```

Add `use crate::units::SOLAR_CONSTANT_W;` to the imports (alongside `G`).

- [ ] **Step 5: Export the helper**

In `crates/wb-astro/src/lib.rs`:

```rust
pub use sky::{SkyModel, daily_mean_factor, hour_angle, tidal_amplitude_m};
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p wb-astro --test tides`
Expected: PASS — all 11 tests.

`earths_spring_tides_are_about_twice_its_neaps` is the load-bearing one. If the ratio comes out at 1.0, the phase doubling is missing — summing amplitudes instead of phasors at `2λ` makes every tide a spring tide.

- [ ] **Step 7: Run the full gate**

Run: `cargo fmt --all && scripts/check.sh`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/wb-astro
git commit -m "$(cat <<'EOF'
feat(astro): tides, phases and apparent brightness

Equilibrium tides from every moon and star, combined as degree-2 phasors so
spring and neap tides emerge from the geometry rather than a special case —
Earth comes out at ~1.5 m springs against ~0.6 m neaps and Aethoria's three
moons beat on a repeatable 30-day cycle. Plus illuminated fraction and a
magnitude-like brightness that ranks the sky.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 9: Dates, weekdays and seasons

Day numbers to dates and back, exactly, across ten million days in either direction, honouring a leap rule nobody on Earth would recognise. Plus which named season a day falls in, wrap-around included.

**Files:**
- Modify: `crates/wb-astro/src/calendar.rs` (date arithmetic)
- Modify: `crates/wb-astro/src/sky.rs` (three delegating methods)
- Modify: `crates/wb-astro/src/lib.rs` (export `Date`)
- Test: `crates/wb-astro/tests/calendar.rs`

**Interfaces:**
- Consumes: `CalendarModel` (Task 6).
- Produces:
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub struct Date {
      pub year: i64,          // 1 at the epoch; 0 and below are before it
      pub month_index: i64,   // the month's own `index` field
      pub day: i64,           // 1-based day of the month
      pub weekday_index: usize,
  }
  impl CalendarModel {
      pub fn is_leap(&self, year: i64) -> bool;
      /// Days in `year`, leap day included.
      pub fn year_length(&self, year: i64) -> i64;
      /// Day index of the first day of `year`. `None` on arithmetic overflow.
      pub fn start_of_year(&self, year: i64) -> Option<i64>;
      pub fn date_of(&self, day_index: i64) -> Date;
      pub fn day_index_of(&self, date: Date) -> Option<i64>;
      /// Day of year (0-based) for a month position and 1-based day.
      pub fn day_of_year(&self, position: usize, day: i64, leap: bool) -> i64;
      /// The inverse: month position and 1-based day for a 0-based day of year.
      pub fn month_day_of(&self, day_of_year: i64, leap: bool) -> (usize, i64);
      pub fn season_of_day(&self, day_of_year: i64) -> Option<&str>;
      /// Replaces Task 6's version: with `leap_rule = every_n` but no `leap_month`, the
      /// leap day goes to the last month.
      pub fn days_in(&self, position: usize, leap: bool) -> i64;
  }
  impl SkyModel {
      pub fn date_of(&self, day_index: i64) -> Date;
      pub fn day_index_of(&self, date: Date) -> Option<i64>;
      pub fn season_of(&self, day: f64) -> Option<&str>;
  }
  ```
  `day_index` is absolute days since the epoch; `day` in `season_of` is a day-of-year, as everywhere else in the API. A date whose month index does not exist, whose day exceeds that month's length, or whose arithmetic overflows gives `None` — never a panic.

- [ ] **Step 1: Write the failing test**

Create `crates/wb-astro/tests/calendar.rs`:

```rust
mod common;

use common::{add, add_aethorian_calendar, aethoria, earth_system, f, i, named, new_log, t};
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
    assert_eq!(sky.calendar().weekdays[sky.date_of(0).weekday_index], "Solday");
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
        vec![("name", t("S")), ("role", t("primary")), ("mass_solar", f(1.0))],
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
            assert_eq!(sky.day_index_of(date), Some(d), "{date:?} did not round-trip");
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
        Date { month_index: 14, ..base },
        Date { month_index: 0, ..base },
        Date { day: 31, ..base },
        Date { day: 0, ..base },
        Date { day: -5, ..base },
        Date { year: i64::MAX, ..base },
        Date { year: i64::MIN, ..base },
    ] {
        assert_eq!(sky.day_index_of(bad), None, "{bad:?} was accepted");
    }
    // The weekday is derived, so a wrong one is ignored rather than rejected.
    assert_eq!(
        sky.day_index_of(Date { weekday_index: 99, ..base }),
        sky.day_index_of(base)
    );
}

#[test]
fn ten_million_days_out_still_round_trips() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    for d in [-10_000_000, -9_999_999, 1_000_001, 10_000_000] {
        assert_eq!(sky.day_index_of(sky.date_of(d)), Some(d));
    }
}

#[test]
fn a_season_is_found_by_its_named_range() {
    let mut log = aethoria();
    let cal = named(&log, "AE"); // the calendar is named by its epoch
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
```

Building the model inside each proptest case is wasteful but correct, and 512 cases of it still run in well under a second — do not restructure it into a lazy static.

`named(&log, "AE")` finds the calendar by its `epoch_name`? No — `named` matches the `name` field, and a calendar has none. Replace that line with a lookup by kind:

```rust
    let cal = log
        .state()
        .live()
        .find(|e| e.kind() == Some("astro.calendar"))
        .map(|e| e.id)
        .expect("the Aethorian calendar");
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p wb-astro --test calendar`
Expected: FAIL — `unresolved import wb_astro::Date`.

- [ ] **Step 3: Replace `days_in` and add the date arithmetic**

In `crates/wb-astro/src/calendar.rs`, add the `Date` type above `impl CalendarModel`:

```rust
/// A date in a world's own calendar. `year` 1 is the epoch year; 0 and below precede it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Date {
    pub year: i64,
    /// The month's own `index` field, not its position.
    pub month_index: i64,
    /// 1-based day of the month.
    pub day: i64,
    pub weekday_index: usize,
}
```

Replace the `days_in` method with:

```rust
    /// Days in one month of a given year.
    ///
    /// The leap day goes to `leap_month`; with `leap_rule = every_n` and no month named,
    /// it goes to the last month of the year, so the year length still adds up.
    pub fn days_in(&self, position: usize, leap: bool) -> i64 {
        let Some(m) = self.months.get(position) else {
            return 0;
        };
        let holds_leap_day = match self.leap_month {
            Some(id) => id == m.id,
            None => position + 1 == self.months.len(),
        };
        m.days + i64::from(leap && holds_leap_day)
    }
```

and add the arithmetic:

```rust
    /// Whether `year` carries a leap day.
    pub fn is_leap(&self, year: i64) -> bool {
        match self.leap_every_n {
            Some(n) if n >= 2 => year.rem_euclid(n) == 0,
            _ => false,
        }
    }

    /// Days in `year`.
    pub fn year_length(&self, year: i64) -> i64 {
        self.month_days() + i64::from(self.is_leap(year))
    }

    /// Day index of the first day of `year`; `None` on overflow.
    ///
    /// `(Y−1)·base + ⌊(Y−1)/n⌋` with Euclidean division, which is correct on both sides
    /// of the epoch: year 0 is a leap year under every rule, and starts one day earlier
    /// than a bare multiple of the common year would put it.
    pub fn start_of_year(&self, year: i64) -> Option<i64> {
        let years = year.checked_sub(1)?;
        let plain = years.checked_mul(self.month_days())?;
        match self.leap_every_n {
            Some(n) if n >= 2 => plain.checked_add(years.div_euclid(n)),
            _ => Some(plain),
        }
    }

    /// The date on `day_index`.
    pub fn date_of(&self, day_index: i64) -> Date {
        let base = self.month_days().max(1);
        let (year, mut rest) = match self.leap_every_n {
            Some(n) if n >= 2 => {
                let cycle = n * base + 1;
                let cycles = day_index.div_euclid(cycle);
                let within = day_index.rem_euclid(cycle);
                // Years 1..n−1 of a cycle are common, so plain division finds the year;
                // the leap day only ever extends the last of them.
                let year_in_cycle = (within / base).min(n - 1);
                (
                    cycles * n + year_in_cycle + 1,
                    within - year_in_cycle * base,
                )
            }
            _ => (day_index.div_euclid(base) + 1, day_index.rem_euclid(base)),
        };
        let leap = self.is_leap(year);
        let mut position = 0;
        while position + 1 < self.months.len() {
            let days = self.days_in(position, leap);
            if rest < days {
                break;
            }
            rest -= days;
            position += 1;
        }
        let weekday_index = if self.weekdays.is_empty() {
            0
        } else {
            day_index.rem_euclid(self.weekdays.len() as i64) as usize
        };
        Date {
            year,
            month_index: self.months.get(position).map(|m| m.index).unwrap_or(1),
            day: rest + 1,
            weekday_index,
        }
    }

    /// The day index of `date`, or `None` if no such day exists.
    ///
    /// `weekday_index` is derived, so a wrong one is ignored rather than rejected.
    pub fn day_index_of(&self, date: Date) -> Option<i64> {
        let position = self.month_position_by_index(date.month_index)?;
        let leap = self.is_leap(date.year);
        if date.day < 1 || date.day > self.days_in(position, leap) {
            return None;
        }
        let start = self.start_of_year(date.year)?;
        let before: i64 = (0..position).map(|p| self.days_in(p, leap)).sum();
        start.checked_add(before)?.checked_add(date.day - 1)
    }

    /// The first month with this `index`.
    pub fn month_position_by_index(&self, index: i64) -> Option<usize> {
        self.months.iter().position(|m| m.index == index)
    }

    /// Day of year (0-based) for a month position and a 1-based day.
    pub fn day_of_year(&self, position: usize, day: i64, leap: bool) -> i64 {
        let before: i64 = (0..position.min(self.months.len()))
            .map(|p| self.days_in(p, leap))
            .sum();
        before + (day - 1).max(0)
    }

    /// Month position and 1-based day for a 0-based day of year.
    pub fn month_day_of(&self, day_of_year: i64, leap: bool) -> (usize, i64) {
        let mut rest = day_of_year.rem_euclid(self.year_length(if leap { 0 } else { 1 }).max(1));
        let mut position = 0;
        while position + 1 < self.months.len() {
            let days = self.days_in(position, leap);
            if rest < days {
                break;
            }
            rest -= days;
            position += 1;
        }
        (position, rest + 1)
    }

    /// The first named season covering this 0-based day of year, wrap-around included.
    pub fn season_of_day(&self, day_of_year: i64) -> Option<&str> {
        let length = self.month_days().max(1);
        let doy = day_of_year.rem_euclid(length);
        for s in &self.seasons {
            let (Some(start_pos), Some(end_pos)) = (
                self.month_position(s.start_month),
                self.month_position(s.end_month),
            ) else {
                continue;
            };
            let start = self.day_of_year(start_pos, s.start_day, false);
            let end = self.day_of_year(end_pos, s.end_day, false);
            let inside = if start <= end {
                (start..=end).contains(&doy)
            } else {
                // Wraps the year end.
                doy >= start || doy <= end
            };
            if inside {
                return Some(s.name.as_str());
            }
        }
        None
    }
```

`month_day_of`'s `year_length(if leap { 0 } else { 1 })` reads badly — it is picking a year of the right leapness to get the right length. Write it as `let length = self.month_days() + i64::from(leap);` instead and use that.

- [ ] **Step 4: Delegate from `SkyModel`**

Add to `impl SkyModel` in `crates/wb-astro/src/sky.rs`:

```rust
    /// The date on an absolute day index since the epoch.
    pub fn date_of(&self, day_index: i64) -> Date {
        self.calendar.date_of(day_index)
    }

    /// The absolute day index of a date, or `None` if no such day exists.
    pub fn day_index_of(&self, date: Date) -> Option<i64> {
        self.calendar.day_index_of(date)
    }

    /// The named season a day-of-year falls in (spec §4.5).
    pub fn season_of(&self, day: f64) -> Option<&str> {
        let doy = if day.is_finite() {
            libm::floor(day) as i64
        } else {
            0
        };
        self.calendar.season_of_day(doy)
    }
```

and `use crate::calendar::Date;` to its imports.

- [ ] **Step 5: Export `Date`**

In `crates/wb-astro/src/lib.rs`:

```rust
pub use calendar::{CalendarModel, Date, MonthView, SeasonView};
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p wb-astro --test calendar`
Expected: PASS — 11 tests plus the two property tests.

`dates_round_trip_over_a_hundred_thousand_consecutive_days` is the gate. If it fails only on leap years, `start_of_year` and `date_of` disagree about which year of a cycle carries the extra day; they are inverses of each other by construction, so fix whichever one departs from `is_leap`.

- [ ] **Step 7: Run the full gate**

Run: `cargo fmt --all && scripts/check.sh`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/wb-astro
git commit -m "$(cat <<'EOF'
feat(astro): dates, weekdays and seasons

date_of and day_index_of are exact inverses across ±10 million days, with an
every-n leap rule that lands on a named month or the year's end, and work
either side of the epoch. season_of resolves named ranges including ones that
wrap the year. An impossible date returns None rather than panicking.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 10: `propose_seasons`

The engine reads the sunlight curve and offers to cut the year into named seasons. These are Constraints suggestions, so the user applies one with a click and then renames it — which is why Task 2 taught suggestions to create entities.

**Files:**
- Create: `crates/wb-astro/src/seasons.rs`
- Modify: `crates/wb-astro/src/sky.rs` (one delegating method)
- Modify: `crates/wb-astro/src/lib.rs`
- Test: `crates/wb-astro/tests/seasons.rs`

**Interfaces:**
- Consumes: `SkyModel::insolation` (Task 7), `CalendarModel` (Task 9), `wb_constraints::{Suggestion, NewEntity, IssueCode, Param}` (Task 2).
- Produces:
  ```rust
  impl SkyModel {
      /// Suggestions that create `n` seasons tiling the year at the sunlight curve's
      /// turning points.
      pub fn propose_seasons(&self, n: usize) -> Vec<Suggestion>;
  }
  /// The day-of-year cuts a proposal would use: `n` strictly increasing values in
  /// `0..year_days`, the k-th season running from `cuts[k]` up to the day before
  /// `cuts[k+1]` (the last wraps).
  pub fn season_cuts(curve: &[f64], n: usize) -> Vec<i64>;
  /// The reference latitude the curve is read at: 45° north, where seasons are clearest.
  pub const PROPOSAL_LAT_DEG: f64 = 45.0;
  ```
  Each suggestion carries `code = "astro.add_season"`, params `name`/`start`/`end`, no
  writes, and one `NewEntity` of kind `astro.season`. Rendering falls through
  `wb_constraints::render_suggestion`'s default (`code key=value`) on purpose: the string
  table in `wb-constraints` stays free of other subsystems' copy, and the Editor will own
  this text when it exists.

- [ ] **Step 1: Write the failing test**

Create `crates/wb-astro/tests/seasons.rs`:

```rust
mod common;

use common::{aethoria, earth_system};
use std::collections::BTreeSet;
use wb_astro::{PROPOSAL_LAT_DEG, SkyModel, season_cuts};
use wb_constraints::{Param, apply_suggestion};
use wb_editlog::Value;

#[test]
fn cuts_are_strictly_increasing_and_inside_the_year() {
    let curve: Vec<f64> = (0..100)
        .map(|d| libm::sin(std::f64::consts::TAU * d as f64 / 100.0))
        .collect();
    for n in 1..=12 {
        let cuts = season_cuts(&curve, n);
        assert_eq!(cuts.len(), n, "n = {n}");
        for w in cuts.windows(2) {
            assert!(w[1] > w[0], "n = {n}: {cuts:?} is not increasing");
        }
        assert!(cuts[0] >= 0 && *cuts.last().unwrap() < 100, "{cuts:?}");
    }
}

#[test]
fn more_seasons_than_days_is_capped_and_zero_is_empty() {
    let curve = vec![1.0, 2.0, 3.0];
    assert_eq!(season_cuts(&curve, 0).len(), 0);
    assert_eq!(season_cuts(&curve, 9).len(), 3);
    assert_eq!(season_cuts(&[], 4).len(), 0);
}

#[test]
fn cuts_land_on_the_curves_turning_points() {
    // A clean one-year sine: peak at 25, trough at 75, steepest at 0 and 50.
    let curve: Vec<f64> = (0..100)
        .map(|d| libm::sin(std::f64::consts::TAU * d as f64 / 100.0))
        .collect();
    let cuts = season_cuts(&curve, 4);
    for c in &cuts {
        let near = [0i64, 25, 50, 75]
            .iter()
            .map(|t| (c - t).abs().min(100 - (c - t).abs()))
            .min()
            .unwrap();
        assert!(near <= 2, "cut at {c} is not near a turning point: {cuts:?}");
    }
}

#[test]
fn four_proposed_seasons_tile_the_aethorian_year_exactly() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let proposals = sky.propose_seasons(4);
    assert_eq!(proposals.len(), 4);

    // Apply them all, then walk every day of the year: exactly one season each.
    let mut log = aethoria();
    for s in &proposals {
        apply_suggestion(&mut log, s).expect("a proposed season must be valid");
    }
    let sky = SkyModel::build(log.state()).unwrap();
    assert_eq!(sky.calendar().seasons.len(), 4);
    for d in 0..390 {
        assert!(
            sky.season_of(d as f64).is_some(),
            "day {d} is in no proposed season"
        );
    }
    // No overlaps: each day's season is the first match, so count distinct runs instead.
    let names: BTreeSet<&str> = (0..390).filter_map(|d| sky.season_of(d as f64)).collect();
    assert_eq!(names.len(), 4, "every proposed season should own some days: {names:?}");
}

#[test]
fn proposals_name_their_months_and_days() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let cal_id = sky.calendar().id;
    for s in sky.propose_seasons(3) {
        assert_eq!(s.code.as_str(), "astro.add_season");
        assert!(s.writes.is_empty());
        assert_eq!(s.creates.len(), 1);
        let e = &s.creates[0];
        assert_eq!(e.kind, "astro.season");
        let field = |name: &str| e.fields.iter().find(|(f, _)| f == name).map(|(_, v)| v);
        assert!(matches!(field("name"), Some(Value::Text(_))));
        assert!(matches!(field("start_day"), Some(Value::Int(d)) if *d >= 1));
        assert!(matches!(field("end_day"), Some(Value::Int(d)) if *d >= 1));
        assert!(matches!(field("start_month"), Some(Value::Entity(_))));
        assert!(matches!(field("end_month"), Some(Value::Entity(_))));
        assert_eq!(field("calendar"), Some(&Value::Entity(cal_id)));
        assert!(matches!(s.params.get("name"), Some(Param::Text(_))));
    }
}

#[test]
fn proposed_names_describe_the_light_and_are_distinct() {
    let log = earth_system();
    let sky = SkyModel::build(log.state()).unwrap();
    let names: Vec<String> = sky
        .propose_seasons(4)
        .iter()
        .filter_map(|s| match s.params.get("name") {
            Some(Param::Text(t)) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(names.len(), 4);
    let unique: BTreeSet<&String> = names.iter().collect();
    assert_eq!(unique.len(), 4, "names must be distinct: {names:?}");
    // Earth at 45°N has a clear high and low season.
    assert!(
        names.iter().any(|n| n.contains("High Sun")),
        "no bright season among {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("Low Sun")),
        "no dark season among {names:?}"
    );
}

#[test]
fn one_proposed_season_covers_the_whole_year() {
    let mut log = aethoria();
    let proposals = SkyModel::build(log.state()).unwrap().propose_seasons(1);
    assert_eq!(proposals.len(), 1);
    apply_suggestion(&mut log, &proposals[0]).unwrap();
    let sky = SkyModel::build(log.state()).unwrap();
    for d in [0, 1, 195, 389] {
        assert!(sky.season_of(d as f64).is_some(), "day {d}");
    }
}

#[test]
fn a_world_with_no_seasonal_variation_still_gets_a_tiling() {
    // Zero tilt: the curve is flat, so there are no turning points to find.
    let mut log = aethoria();
    common::set(
        &mut log,
        wb_editlog::EntityId::PLANET,
        vec![("axial_tilt_deg", common::f(0.0)), ("eccentricity", common::f(0.0))],
    );
    let sky = SkyModel::build(log.state()).unwrap();
    let proposals = sky.propose_seasons(4);
    assert_eq!(proposals.len(), 4);
    let mut applied = log;
    for s in &proposals {
        apply_suggestion(&mut applied, s).unwrap();
    }
    let sky = SkyModel::build(applied.state()).unwrap();
    for d in 0..390 {
        assert!(sky.season_of(d as f64).is_some(), "day {d} uncovered");
    }
    assert!(PROPOSAL_LAT_DEG > 0.0);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p wb-astro --test seasons`
Expected: FAIL — `unresolved imports wb_astro::season_cuts, wb_astro::PROPOSAL_LAT_DEG`.

- [ ] **Step 3: Write `seasons.rs`**

Create `crates/wb-astro/src/seasons.rs`:

```rust
//! Reading seasons off the sunlight curve.
//!
//! The user owns their seasons: any number, any lengths, their own names. This only
//! offers a starting point — where the light turns — as suggestions they apply and then
//! rename.

use crate::sky::SkyModel;
use std::collections::BTreeMap;
use wb_constraints::{IssueCode, NewEntity, Param, Suggestion};
use wb_editlog::Value;
use wb_grid::LatLon;

/// The latitude the curve is read at. Mid-latitudes have the clearest seasons: the
/// tropics barely vary and a pole is all-or-nothing.
pub const PROPOSAL_LAT_DEG: f64 = 45.0;

/// Days of the year where the curve turns: local extrema, and the steepest day of each
/// arc between them. Circular — day 0 follows the last day.
fn turning_points(curve: &[f64]) -> Vec<i64> {
    let n = curve.len();
    if n < 3 {
        return Vec::new();
    }
    let at = |i: usize| curve[i % n];
    let slope = |i: usize| at(i + 1) - at(i + n - 1);
    let mut extrema = Vec::new();
    for i in 0..n {
        let before = at(i) - at(i + n - 1);
        let after = at(i + 1) - at(i);
        if (before > 0.0 && after <= 0.0) || (before < 0.0 && after >= 0.0) {
            extrema.push(i as i64);
        }
    }
    let mut points = extrema.clone();
    // The steepest day between each pair of neighbouring extrema.
    if extrema.len() >= 2 {
        for pair in extrema.windows(2) {
            let (a, b) = (pair[0] as usize, pair[1] as usize);
            let mut best = (f64::MIN, a);
            for i in a..=b {
                let s = slope(i).abs();
                if s > best.0 {
                    best = (s, i);
                }
            }
            points.push(best.1 as i64);
        }
    }
    points.sort_unstable();
    points.dedup();
    points
}

/// `n` strictly increasing cut days in `0..curve.len()`.
///
/// Each cut is the turning point nearest its evenly spaced ideal, inside a window that
/// keeps the cuts in order and leaves room for the ones after it; with no candidate in
/// range the ideal itself is used. That gives exactly `n` distinct cuts, so the seasons
/// always tile the year — a proposal with a gap or an overlap would be worse than no
/// proposal at all.
pub fn season_cuts(curve: &[f64], n: usize) -> Vec<i64> {
    let length = curve.len() as i64;
    if n == 0 || length == 0 {
        return Vec::new();
    }
    let n = (n as i64).min(length);
    let candidates = turning_points(curve);
    let mut cuts: Vec<i64> = Vec::new();
    for k in 0..n {
        let ideal = k * length / n;
        let lower = cuts.last().map(|c| c + 1).unwrap_or(0);
        let upper = length - (n - k);
        let window = (length / (2 * n)).max(1);
        let pick = candidates
            .iter()
            .copied()
            .filter(|c| *c >= lower && *c <= upper && (*c - ideal).abs() <= window)
            .min_by_key(|c| ((*c - ideal).abs(), *c))
            .unwrap_or_else(|| ideal.clamp(lower, upper));
        cuts.push(pick);
    }
    cuts
}

/// A descriptive placeholder name for a range of the curve.
fn label(curve: &[f64], start: i64, end_exclusive: i64, mean: f64, amplitude: f64) -> &'static str {
    let n = curve.len() as i64;
    if n == 0 {
        return "Season";
    }
    let at = |i: i64| curve[(i.rem_euclid(n)) as usize];
    let net = at(end_exclusive - 1) - at(start);
    let span = (end_exclusive - start).max(1);
    let sum: f64 = (start..end_exclusive).map(at).sum();
    let level = sum / span as f64;
    let threshold = amplitude * 0.1;
    if net > threshold {
        "Rising Light"
    } else if net < -threshold {
        "Falling Light"
    } else if level >= mean {
        "High Sun"
    } else {
        "Low Sun"
    }
}

/// Suggestions creating `n` seasons that tile the year (spec §4.5).
pub fn propose(sky: &SkyModel, n: usize) -> Vec<Suggestion> {
    let cal = sky.calendar();
    let length = cal.month_days();
    if n == 0 || length <= 0 {
        return Vec::new();
    }
    let lat = LatLon::from_degrees(PROPOSAL_LAT_DEG, 0.0);
    let curve: Vec<f64> = (0..length).map(|d| sky.insolation(lat, d as f64)).collect();
    let cuts = season_cuts(&curve, n);
    if cuts.is_empty() {
        return Vec::new();
    }
    let mean = curve.iter().sum::<f64>() / curve.len() as f64;
    let amplitude = {
        let hi = curve.iter().copied().fold(f64::MIN, f64::max);
        let lo = curve.iter().copied().fold(f64::MAX, f64::min);
        (hi - lo).max(0.0)
    };
    let mut used: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut out = Vec::new();
    for (k, start) in cuts.iter().copied().enumerate() {
        // The next cut, wrapped: the last season runs to the day before the first cut.
        let next = if k + 1 < cuts.len() {
            cuts[k + 1]
        } else {
            cuts[0] + length
        };
        let end = next - 1;
        let (start_pos, start_day) = cal.month_day_of(start, false);
        let (end_pos, end_day) = cal.month_day_of(end, false);
        let (Some(start_month), Some(end_month)) = (
            cal.months.get(start_pos).map(|m| m.id),
            cal.months.get(end_pos).map(|m| m.id),
        ) else {
            continue;
        };
        let base = label(&curve, start, next, mean, amplitude);
        let count = used.entry(base).or_insert(0);
        *count += 1;
        let name = if *count == 1 {
            base.to_string()
        } else {
            format!("{base} {count}")
        };
        let mut params = BTreeMap::new();
        params.insert("name".to_string(), Param::Text(name.clone()));
        params.insert("start".to_string(), Param::Int(start));
        params.insert("end".to_string(), Param::Int(end.rem_euclid(length)));
        out.push(Suggestion {
            code: IssueCode::new("astro.add_season"),
            params,
            writes: Vec::new(),
            creates: vec![NewEntity {
                kind: "astro.season".to_string(),
                fields: vec![
                    ("name".to_string(), Value::Text(name)),
                    ("start_month".to_string(), Value::Entity(start_month)),
                    ("start_day".to_string(), Value::Int(start_day)),
                    ("end_month".to_string(), Value::Entity(end_month)),
                    ("end_day".to_string(), Value::Int(end_day)),
                    ("calendar".to_string(), Value::Entity(cal.id)),
                ],
            }],
        });
    }
    out
}
```

- [ ] **Step 4: Delegate from `SkyModel`**

Add to `impl SkyModel`:

```rust
    /// Suggestions that create `n` seasons tiling the year at the sunlight curve's
    /// turning points (spec §4.5).
    pub fn propose_seasons(&self, n: usize) -> Vec<Suggestion> {
        crate::seasons::propose(self, n)
    }
```

and `use wb_constraints::Suggestion;` to its imports.

- [ ] **Step 5: Export it**

In `crates/wb-astro/src/lib.rs`, add `mod seasons;` and:

```rust
pub use seasons::{PROPOSAL_LAT_DEG, season_cuts};
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p wb-astro --test seasons`
Expected: PASS — all 8 tests.

`four_proposed_seasons_tile_the_aethorian_year_exactly` is the one that matters: a proposal with a gap or an overlap is worse than no proposal, and `season_cuts`' window arithmetic is what prevents both.

- [ ] **Step 7: Run the full gate**

Run: `cargo fmt --all && scripts/check.sh`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/wb-astro
git commit -m "$(cat <<'EOF'
feat(astro): propose_seasons from the sunlight curve

Reads insolation at 45° through the year, finds the extrema and the steepest
days between them, and cuts the year into n ranges at the turning point
nearest each evenly spaced ideal. The cuts are strictly increasing by
construction, so proposed seasons always tile the year exactly. Each proposal
is a Constraints suggestion that creates an astro.season entity.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 11: Orbital feasibility checkers

Five checkers over the system's geometry: a companion star that wrecks the planets, a stated period that contradicts its own distance, bodies numbered against their distances, orbits that cross, moons that cannot survive, and a belt sitting on someone else's orbit. Every finding carries the numbers that produced it.

**Files:**
- Create: `crates/wb-astro/src/checks/mod.rs`
- Create: `crates/wb-astro/src/checks/orbital.rs`
- Modify: `crates/wb-astro/src/lib.rs`
- Modify: `crates/wb-astro/tests/common/mod.rs` (the evaluation harness)
- Test: `crates/wb-astro/tests/checks_orbital.rs`

**Interfaces:**
- Consumes: `SkyModel`, `BodyView` (Task 6), `wb_constraints::{Checker, CheckContext, KindPattern, Finding, Param, Suggestion, IssueCode}`.
- Produces:
  ```rust
  pub struct CompanionCheck;  // id "astro.companion",  kinds: Exact("astro.star")
  pub struct OrderingCheck;   // id "astro.ordering",   kinds: Exact("astro.body")
  pub struct OrbitCheck;      // id "astro.orbits",     kinds: Exact("astro.body")
  pub struct MoonCheck;       // id "astro.moons",      kinds: Exact("astro.body")
  pub struct BeltCheck;       // id "astro.belt",       kinds: Exact("astro.body")
  /// Registers every astro checker into a registry.
  pub fn with_astro_checks(registry: &mut CheckerRegistry);
  /// Holman–Wiegert critical semi-major axis for an S-type orbit in a binary.
  pub fn critical_distance_m(separation_m: f64, mass_ratio: f64, eccentricity: f64) -> f64;
  /// Hill radius of a body of `mass_kg` at `a_m` from a primary of `primary_kg`.
  pub fn hill_radius_m(a_m: f64, mass_kg: f64, primary_kg: f64) -> f64;
  /// Roche limit for a satellite of `satellite_density` about a body of `radius_m`.
  pub fn roche_limit_m(radius_m: f64, body_density: f64, satellite_density: f64) -> f64;
  /// `Some((p, q))` when `ratio` is within 2% of `p/q` for small `p`, `q`.
  pub fn simple_ratio(ratio: f64) -> Option<(i64, i64)>;
  ```
  Task 12 extends `with_astro_checks` with the remaining four checkers; these five are all
  it registers for now.

  Issue codes and badness, exactly as spec §6: `astro.companion_destabilizes` 0.9,
  `astro.companion_period_mismatch` 0.8, `astro.order_mismatch` `min(1, places/4)`,
  `astro.orbit_crossing` 0.5, `astro.resonance_protects` (support),
  `astro.resonance` (support), `astro.moon_outside_hill` 0.9,
  `astro.moon_inside_roche` 0.9, `astro.belt_overlaps_orbit` 0.5.

- [ ] **Step 1: Add the evaluation harness to the fixtures**

Append to `crates/wb-astro/tests/common/mod.rs`:

```rust
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
```

- [ ] **Step 2: Write the failing test**

Create `crates/wb-astro/tests/checks_orbital.rs`:

```rust
mod common;

use common::{add, add_aethorian_calendar, aethoria, astro_report, codes, earth_system, f, findings, i, named, new_log, set, t};
use wb_astro::{
    critical_distance_m, hill_radius_m, roche_limit_m, simple_ratio,
};
use wb_constraints::{FindingKind, Param};
use wb_editlog::{EntityId, Value};

fn close(got: f64, want: f64, rel: f64) {
    let err = (got - want).abs() / want.abs();
    assert!(err <= rel, "got {got}, want {want} (relative error {err} > {rel})");
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
    // The Moon's Roche limit about Earth is about 9 500 km.
    close(roche_limit_m(6.371e6, 5_514.0, 3_344.0), 9.5e6, 0.05);
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
    assert!(matches!(finding.params.get("critical_au"), Some(Param::Float(_))));
    // Both suggested fixes: move it out, or make it too light to be a star.
    assert_eq!(finding.suggestions.len(), 2);
    let codes: Vec<&str> = finding.suggestions.iter().map(|s| s.code.as_str()).collect();
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
    assert!(matches!(found[0].params.get("stated_period_years"), Some(Param::Float(y)) if (*y - 1.0).abs() < 1e-9));
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
    let found = findings(&astro_report(&log), "astro.companion_period_mismatch");
    assert!(found.is_empty(), "253 years at 40 AU is within 25%: {found:?}");
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
    let found = findings(&astro_report(&log), "astro.companion_period_mismatch");
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
    let found = findings(&astro_report(&log), "astro.order_mismatch");
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
    assert!(matches!(protects[0].params.get("p"), Some(Param::Int(3))));
    assert!(matches!(protects[0].params.get("q"), Some(Param::Int(2))));
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
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p wb-astro --test checks_orbital`
Expected: FAIL — `unresolved imports wb_astro::with_astro_checks, wb_astro::critical_distance_m, ...`.

- [ ] **Step 4: Write `checks/orbital.rs`**

Create `crates/wb-astro/src/checks/orbital.rs`:

```rust
//! Checkers over the system's geometry.
//!
//! Each one builds a [`SkyModel`] from `ctx.state` inside `check`. That rebuild per entity
//! is deliberate: a checker holds no state, so it cannot go stale against the world it is
//! judging, and a system is tens of bodies. If it ever shows up in the perf guard, the fix
//! is the memo slot recorded in the Constraints follow-ups, not a cache in here.

use crate::sky::SkyModel;
use crate::system::BodyView;
use crate::units::{AU_M, MIN_STELLAR_MASS_SOLAR};
use std::collections::BTreeMap;
use wb_constraints::{
    CheckContext, Checker, ConstraintView, Finding, IssueCode, KindPattern, Param, Suggestion,
};
use wb_editlog::{EntityId, Value};

/// Holman–Wiegert (1999) critical semi-major axis for an S-type orbit, as a fraction of
/// the binary separation.
///
/// Exposed separately because the fix for an unstable companion has to invert the
/// criterion: the separation that makes a given orbit safe is that orbit divided by this
/// fraction, and multiplying the separation by a guessed factor does not reliably clear
/// the finding it is offered for.
pub fn critical_fraction(mass_ratio: f64, eccentricity: f64) -> f64 {
    let u = mass_ratio.clamp(0.0, 1.0);
    let e = eccentricity.clamp(0.0, 0.95);
    let fraction =
        0.464 - 0.380 * u - 0.631 * e + 0.586 * u * e + 0.150 * e * e - 0.198 * u * e * e;
    if fraction.is_finite() { fraction.max(0.0) } else { 0.0 }
}

/// The critical distance itself: the separation times [`critical_fraction`].
pub fn critical_distance_m(separation_m: f64, mass_ratio: f64, eccentricity: f64) -> f64 {
    let d = separation_m * critical_fraction(mass_ratio, eccentricity);
    if d.is_finite() { d.max(0.0) } else { 0.0 }
}

/// Hill radius: `a·∛(m / 3M)`.
pub fn hill_radius_m(a_m: f64, mass_kg: f64, primary_kg: f64) -> f64 {
    if !(a_m > 0.0) || !(mass_kg > 0.0) || !(primary_kg > 0.0) {
        return 0.0;
    }
    let r = a_m * libm::cbrt(mass_kg / (3.0 * primary_kg));
    if r.is_finite() { r.max(0.0) } else { 0.0 }
}

/// Roche limit for a rigid-free satellite: `2.44·R·∛(ρ_body / ρ_satellite)`.
pub fn roche_limit_m(radius_m: f64, body_density: f64, satellite_density: f64) -> f64 {
    if !(radius_m > 0.0) || !(body_density > 0.0) || !(satellite_density > 0.0) {
        return 0.0;
    }
    let d = 2.44 * radius_m * libm::cbrt(body_density / satellite_density);
    if d.is_finite() { d.max(0.0) } else { 0.0 }
}

/// Tolerance on a resonance, as a fraction of the ratio.
const RESONANCE_TOLERANCE: f64 = 0.02;

/// `Some((p, q))` when `ratio` is within 2% of `p/q` for `p`, `q` in 1..=5.
///
/// Smaller denominators win, so a 2:1 is never reported as a 4:2.
pub fn simple_ratio(ratio: f64) -> Option<(i64, i64)> {
    if !ratio.is_finite() || ratio <= 0.0 {
        return None;
    }
    let mut best: Option<(i64, i64)> = None;
    for q in 1..=5i64 {
        for p in 1..=5i64 {
            if p == q && p != 1 {
                continue;
            }
            let target = p as f64 / q as f64;
            if (ratio - target).abs() <= RESONANCE_TOLERANCE * target {
                let better = match best {
                    None => true,
                    Some((bp, bq)) => (q, p) < (bq, bp),
                };
                if better {
                    best = Some((p, q));
                }
            }
        }
    }
    best
}

fn model(ctx: &CheckContext<'_>) -> Option<SkyModel> {
    SkyModel::build(ctx.state).ok()
}

/// Bodies sharing a parent, in `EntityId` order, belts excluded.
fn siblings<'a>(sky: &'a SkyModel, parent: Option<EntityId>) -> Vec<&'a BodyView> {
    sky.bodies()
        .filter(|b| b.parent == parent && b.extent_m.is_none() && b.a_m > 0.0)
        .collect()
}

fn au(x: f64) -> Param {
    Param::Float(x / AU_M)
}

fn write(entity: EntityId, field: &str, value: Value) -> Vec<(EntityId, String, Value)> {
    vec![(entity, field.to_string(), value)]
}

fn suggestion(code: &str, params: Vec<(&str, Param)>, writes: Vec<(EntityId, String, Value)>) -> Suggestion {
    Suggestion {
        code: IssueCode::new(code),
        params: params
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect::<BTreeMap<String, Param>>(),
        writes,
        creates: Vec::new(),
    }
}

/// Companion-star stability and the period it claims.
pub struct CompanionCheck;

impl Checker for CompanionCheck {
    fn id(&self) -> &'static str {
        "astro.companion"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("astro.star")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = model(ctx) else {
            return Vec::new();
        };
        let Some(star) = sky.body(c.id) else {
            return Vec::new();
        };
        if star.is_primary_star {
            return Vec::new();
        }
        let primary = sky.primary_star();
        let mut out = Vec::new();

        // Stability: everything orbiting the primary beyond the critical distance is at
        // risk, and so is anything inside the companion's own reach.
        if star.a_m > 0.0 {
            let ratio = star.mass_kg / (star.mass_kg + primary.mass_kg).max(f64::MIN_POSITIVE);
            let fraction = critical_fraction(ratio, star.eccentricity);
            let critical = critical_distance_m(star.a_m, ratio, star.eccentricity);
            let at_risk: Vec<&BodyView> = siblings(&sky, Some(primary.id))
                .into_iter()
                .filter(|b| b.id != star.id && b.aphelion_m() > critical)
                .collect();
            if !at_risk.is_empty() {
                let outermost = at_risk
                    .iter()
                    .map(|b| b.aphelion_m())
                    .fold(0.0_f64, f64::max);
                let names = at_risk
                    .iter()
                    .map(|b| b.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                // The separation that puts the outermost body inside the critical
                // distance, with a 20% margin — inverting the criterion rather than
                // guessing a multiple, so applying the fix really does clear the finding.
                let safe = if fraction > 0.0 {
                    outermost / fraction * 1.2
                } else {
                    outermost * 10.0
                };
                let mut f = Finding::issue(self.id(), "astro.companion_destabilizes", 0.9)
                    .with_param("critical_au", au(critical))
                    .with_param("distance_au", au(star.a_m))
                    .with_param("bodies", Param::Text(names))
                    .with_param("count", Param::Int(at_risk.len() as i64))
                    .with_suggestion(suggestion(
                        "astro.move_companion_out",
                        vec![("distance_au", au(safe))],
                        write(star.id, "distance_au", Value::Float(safe / AU_M)),
                    ))
                    .with_suggestion(suggestion(
                        "astro.demote_companion",
                        vec![("mass_solar", Param::Float(MIN_STELLAR_MASS_SOLAR))],
                        write(
                            star.id,
                            "mass_solar",
                            Value::Float(MIN_STELLAR_MASS_SOLAR),
                        ),
                    ));
                for b in at_risk {
                    f = f.with_related(b.id);
                }
                out.push(f);
            }
        }

        // The period it claims against the period its distance implies.
        let year_s = sky.year_days() * sky.day_hours() * crate::units::SECONDS_PER_HOUR;
        match (star.stated_a_m, star.stated_period_s) {
            (Some(a), Some(p)) => {
                let implied_period = crate::kepler::period_s(a, primary.mass_kg + star.mass_kg);
                let implied_a =
                    crate::kepler::semi_major_m(p, primary.mass_kg + star.mass_kg);
                if implied_period > 0.0
                    && (p - implied_period).abs() > 0.25 * implied_period
                {
                    out.push(
                        Finding::issue(self.id(), "astro.companion_period_mismatch", 0.8)
                            .with_param("stated_period_years", Param::Float(p / year_s))
                            .with_param(
                                "implied_period_years",
                                Param::Float(implied_period / year_s),
                            )
                            .with_param("stated_distance_au", au(a))
                            .with_param("implied_distance_au", au(implied_a)),
                    );
                }
            }
            (None, Some(p)) => {
                let implied_a =
                    crate::kepler::semi_major_m(p, primary.mass_kg + star.mass_kg);
                let inside = siblings(&sky, Some(primary.id))
                    .into_iter()
                    .filter(|b| b.id != star.id)
                    .any(|b| b.aphelion_m() > implied_a);
                if inside && implied_a > 0.0 {
                    out.push(
                        Finding::issue(self.id(), "astro.companion_period_mismatch", 0.8)
                            .with_param("stated_period_years", Param::Float(p / year_s))
                            .with_param("implied_period_years", Param::Float(p / year_s))
                            .with_param("implied_distance_au", au(implied_a))
                            .with_param("stated_distance_au", au(implied_a)),
                    );
                }
            }
            _ => {}
        }
        out
    }
}

/// Stated `order` against derived distance.
pub struct OrderingCheck;

impl Checker for OrderingCheck {
    fn id(&self) -> &'static str {
        "astro.ordering"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("astro.body")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = model(ctx) else {
            return Vec::new();
        };
        let Some(body) = sky.body(c.id) else {
            return Vec::new();
        };
        // Every sibling, belts included: a belt occupies a place in the numbering.
        let mut group: Vec<&BodyView> = sky
            .bodies()
            .filter(|b| b.parent == body.parent && b.a_m > 0.0)
            .collect();
        if group.len() < 2 {
            return Vec::new();
        }
        let mut by_order = group.clone();
        by_order.sort_by(|a, b| (a.order, a.id).cmp(&(b.order, b.id)));
        group.sort_by(|a, b| {
            a.a_m
                .partial_cmp(&b.a_m)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.id.cmp(&b.id))
        });
        let stated = by_order.iter().position(|b| b.id == body.id);
        let actual = group.iter().position(|b| b.id == body.id);
        let (Some(stated), Some(actual)) = (stated, actual) else {
            return Vec::new();
        };
        if stated == actual {
            return Vec::new();
        }
        let places = stated.abs_diff(actual) as f64;
        let correct = group
            .iter()
            .position(|b| b.id == body.id)
            .map(|p| p as i64 + 1)
            .unwrap_or(body.order);
        vec![
            Finding::issue(
                self.id(),
                "astro.order_mismatch",
                (places / 4.0).min(1.0),
            )
            .with_param("stated", Param::Int(body.order))
            .with_param("places", Param::Int(places as i64))
            .with_param("suggested", Param::Int(correct))
            .with_suggestion(suggestion(
                "astro.renumber",
                vec![("order", Param::Int(correct))],
                write(body.id, "order", Value::Int(correct)),
            )),
        ]
    }
}

/// Orbits that cross, and resonances that excuse them.
pub struct OrbitCheck;

impl Checker for OrbitCheck {
    fn id(&self) -> &'static str {
        "astro.orbits"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("astro.body")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = model(ctx) else {
            return Vec::new();
        };
        let Some(body) = sky.body(c.id) else {
            return Vec::new();
        };
        if body.extent_m.is_some() || body.a_m <= 0.0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for other in siblings(&sky, body.parent) {
            if other.id == body.id {
                continue;
            }
            // Each pair of bodies is judged once, from the lower id. A star or the home
            // world never runs this checker itself (their kinds are not `astro.body`), so
            // a pair involving one of them is judged from whichever body does run —
            // otherwise a companion star sitting on a planet's orbit would go unreported.
            if other.id < body.id && !other.is_star() && !other.is_home {
                continue;
            }
            let overlap = body.perihelion_m() <= other.aphelion_m()
                && other.perihelion_m() <= body.aphelion_m();
            let ratio = if other.period_s > 0.0 {
                body.period_s / other.period_s
            } else {
                0.0
            };
            let resonance = simple_ratio(ratio);
            match (overlap, resonance) {
                (true, Some((p, q))) => out.push(
                    Finding::support(self.id(), "astro.resonance_protects")
                        .with_param("other", Param::Text(other.name.clone()))
                        .with_param("p", Param::Int(p))
                        .with_param("q", Param::Int(q))
                        .with_related(other.id),
                ),
                (true, None) => out.push(
                    Finding::issue(self.id(), "astro.orbit_crossing", 0.5)
                        .with_param("other", Param::Text(other.name.clone()))
                        .with_param("perihelion_au", au(body.perihelion_m()))
                        .with_param("aphelion_au", au(body.aphelion_m()))
                        .with_param("other_perihelion_au", au(other.perihelion_m()))
                        .with_param("other_aphelion_au", au(other.aphelion_m()))
                        .with_related(other.id),
                ),
                (false, Some((p, q))) => out.push(
                    Finding::support(self.id(), "astro.resonance")
                        .with_param("other", Param::Text(other.name.clone()))
                        .with_param("p", Param::Int(p))
                        .with_param("q", Param::Int(q))
                        .with_related(other.id),
                ),
                (false, None) => {}
            }
        }
        out
    }
}

/// Moons that cannot stay where they are.
pub struct MoonCheck;

impl Checker for MoonCheck {
    fn id(&self) -> &'static str {
        "astro.moons"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("astro.body")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = model(ctx) else {
            return Vec::new();
        };
        let Some(moon) = sky.body(c.id) else {
            return Vec::new();
        };
        if moon.class != crate::units::BodyClass::Moon || moon.a_m <= 0.0 {
            return Vec::new();
        }
        let Some(planet) = moon.parent.and_then(|p| sky.body(p)) else {
            return Vec::new();
        };
        let primary = sky.primary_star();
        let mut out = Vec::new();

        let hill = hill_radius_m(planet.a_m, planet.mass_kg, primary.mass_kg);
        if hill > 0.0 && moon.a_m > hill / 3.0 {
            out.push(
                Finding::issue(self.id(), "astro.moon_outside_hill", 0.9)
                    .with_param("distance_km", Param::Float(moon.a_m / 1_000.0))
                    .with_param("hill_third_km", Param::Float(hill / 3_000.0))
                    .with_related(planet.id),
            );
        }
        let roche = roche_limit_m(planet.radius_m, planet.density_kgm3, moon.density_kgm3);
        if roche > 0.0 && moon.perihelion_m() < roche {
            out.push(
                Finding::issue(self.id(), "astro.moon_inside_roche", 0.9)
                    .with_param("distance_km", Param::Float(moon.perihelion_m() / 1_000.0))
                    .with_param("roche_km", Param::Float(roche / 1_000.0))
                    .with_related(planet.id),
            );
        }
        out
    }
}

/// A belt lying across someone else's orbit.
pub struct BeltCheck;

impl Checker for BeltCheck {
    fn id(&self) -> &'static str {
        "astro.belt"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("astro.body")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = model(ctx) else {
            return Vec::new();
        };
        let Some(belt) = sky.body(c.id) else {
            return Vec::new();
        };
        let Some((inner, outer)) = belt.extent_m else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for other in siblings(&sky, belt.parent) {
            if other.perihelion_m() >= inner && other.aphelion_m() <= outer {
                out.push(
                    Finding::issue(self.id(), "astro.belt_overlaps_orbit", 0.5)
                        .with_param("other", Param::Text(other.name.clone()))
                        .with_param("inner_au", au(inner))
                        .with_param("outer_au", au(outer))
                        .with_related(other.id),
                );
            }
        }
        out
    }
}
```

- [ ] **Step 5: Write `checks/mod.rs`**

Create `crates/wb-astro/src/checks/mod.rs`:

```rust
//! Feasibility checkers (spec §6). Every finding carries the numbers that produced it,
//! and every issue can be kept anyway like any other constraint.

mod orbital;

pub use orbital::{
    BeltCheck, CompanionCheck, MoonCheck, OrbitCheck, OrderingCheck, critical_distance_m,
    critical_fraction, hill_radius_m, roche_limit_m, simple_ratio,
};

use wb_constraints::{Checker, CheckerRegistry};

/// Registers every astro checker.
pub fn with_astro_checks(registry: &mut CheckerRegistry) {
    for c in [
        Box::new(BeltCheck) as Box<dyn Checker>,
        Box::new(CompanionCheck),
        Box::new(MoonCheck),
        Box::new(OrbitCheck),
        Box::new(OrderingCheck),
    ] {
        registry
            .register(c)
            .expect("astro checker ids are unique");
    }
}
```

- [ ] **Step 6: Export it**

In `crates/wb-astro/src/lib.rs`, add `mod checks;` and:

```rust
pub use checks::{
    BeltCheck, CompanionCheck, MoonCheck, OrbitCheck, OrderingCheck, critical_distance_m,
    critical_fraction, hill_radius_m, roche_limit_m, simple_ratio, with_astro_checks,
};
```

- [ ] **Step 7: Run the tests**

Run: `cargo test -p wb-astro --test checks_orbital`
Expected: PASS — all 17 tests.

Two to watch. `applying_the_fix_clears_the_finding` is the real test of the suggestion: if three times the outermost aphelion is still inside the critical distance, the multiplier is wrong, not the criterion. And `neptune_and_pluto_are_protected_by_their_resonance` is the spec's named case — if it reports a crossing, check `simple_ratio`'s tolerance against the real 1.5033 period ratio.

- [ ] **Step 8: Run the full gate**

Run: `cargo fmt --all && scripts/check.sh`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/wb-astro
git commit -m "$(cat <<'EOF'
feat(astro): orbital feasibility checkers

Five checkers with the published criteria behind them: Holman-Wiegert for a
companion star's reach, Hill radius and Roche limit for moons, numbering
against derived distance, crossing orbits excused by a simple-ratio resonance,
and belts lying across orbits. Findings carry their numbers; the companion and
numbering findings carry one-click fixes that actually clear them.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 12: World and calendar checkers, and `register`

The remaining four checkers — a body that states a period and a distance that disagree, a world too hot or cold for liquid water, a day about to be tidally locked, and a calendar whose months do not add up — and the single entry point the rest of the engine calls.

**Files:**
- Create: `crates/wb-astro/src/checks/world.rs`
- Create: `crates/wb-astro/src/checks/calendar.rs`
- Modify: `crates/wb-astro/src/checks/mod.rs` (register the four)
- Modify: `crates/wb-astro/src/lib.rs` (add `register`)
- Test: `crates/wb-astro/tests/checks_world.rs`

**Interfaces:**
- Consumes: everything from Tasks 6–11.
- Produces:
  ```rust
  pub struct BodyCheck;      // id "astro.body",     kinds: Exact("astro.body")
  pub struct ClimateCheck;   // id "astro.climate",  kinds: Exact("planet")
  pub struct SpinCheck;      // id "astro.spin",     kinds: Exact("planet")
  pub struct CalendarCheck;  // id "astro.calendar", kinds: Exact("astro.calendar")
  /// Mean flux at the home world over one year, W·m⁻², summed over every star.
  pub fn mean_stellar_flux(sky: &SkyModel) -> f64;
  /// Equilibrium temperature in kelvin for a flux and an albedo.
  pub fn equilibrium_temperature_k(flux: f64, albedo: f64) -> f64;
  /// Tidal-locking timescale in seconds for the home world under one perturber.
  pub fn locking_time_s(sky: &SkyModel, perturber_kg: f64, distance_m: f64) -> f64;
  /// The one call the rest of the engine makes (spec §5).
  pub fn register(log: &mut EditLog, registry: &mut CheckerRegistry, catalog: &mut KindCatalog);
  ```
  Codes and badness from spec §6: `astro.period_distance_conflict` 0.7,
  `astro.outside_habitable_zone` 0.6, `astro.tidal_locking` (consequence),
  `astro.calendar_sum_mismatch` 0.9, `astro.leap_drift` 0.6. The habitable band is
  250–320 K; `astro.leap_drift` fires above 0.01 days per year of residual drift, and
  `astro.calendar_sum_mismatch` takes over above half a day, so the two never both fire.

- [ ] **Step 1: Write the failing test**

Create `crates/wb-astro/tests/checks_world.rs`:

```rust
mod common;

use common::{add, aethoria, astro_report, codes, earth_system, f, findings, i, named, set, t};
use wb_astro::{
    SkyModel, equilibrium_temperature_k, locking_time_s, mean_stellar_flux,
};
use wb_constraints::{FindingKind, Param};
use wb_editlog::{EntityId, Value};

fn close(got: f64, want: f64, rel: f64) {
    let err = (got - want).abs() / want.abs();
    assert!(err <= rel, "got {got}, want {want} (relative error {err} > {rel})");
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
    let found = findings(&astro_report(&log), "astro.outside_habitable_zone");
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
    assert!((250.0..=320.0).contains(&t), "Aethoria's equilibrium temperature = {t} K");
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
    for key in ["stated_period_years", "implied_period_years", "stated_distance_au"] {
        assert!(found[0].params.contains_key(key), "missing {key}");
    }
    // Two ways to resolve it: keep the distance, or keep the period.
    let codes: Vec<&str> = found[0].suggestions.iter().map(|s| s.code.as_str()).collect();
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
        log.state().field(named(&log, "Contradiction"), "period_years"),
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
    assert!(matches!(found[0].params.get("month_total"), Some(Param::Float(d)) if (*d - 390.0).abs() < 1e-9));
    assert!(matches!(found[0].params.get("year_days"), Some(Param::Float(d)) if (*d - 400.0).abs() < 1e-9));
    // The fix lengthens the longest month.
    let fix = found[0].suggestions[0].clone();
    assert_eq!(fix.code.as_str(), "astro.adjust_longest_month");
    assert_eq!(fix.writes[0].1, "days");
    wb_constraints::apply_suggestion(&mut log, &fix).unwrap();
    assert!(findings(&astro_report(&log), "astro.calendar_sum_mismatch").is_empty());
    assert_eq!(SkyModel::build(log.state()).unwrap().calendar().month_days(), 400);
}

#[test]
fn a_calendar_that_adds_up_raises_nothing() {
    let r = astro_report(&aethoria());
    assert!(findings(&r, "astro.calendar_sum_mismatch").is_empty(), "{:?}", codes(&r));
    assert!(findings(&r, "astro.leap_drift").is_empty());
    // Earth: 365 days plus a leap day every 4 years drifts only 0.0078 days a year.
    let r = astro_report(&earth_system());
    assert!(findings(&r, "astro.calendar_sum_mismatch").is_empty(), "{:?}", codes(&r));
    assert!(findings(&r, "astro.leap_drift").is_empty());
}

#[test]
fn a_leap_rule_that_still_drifts_is_reported() {
    // 365 days with no leap rule at all against a 365.2422-day year: a quarter day a year.
    let mut log = common::new_log();
    add(
        &mut log,
        "astro.star",
        vec![("name", t("S")), ("role", t("primary")), ("mass_solar", f(1.0))],
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
    assert!(matches!(found[0].params.get("drift_days"), Some(Param::Float(d)) if (*d - 0.2422).abs() < 0.01));
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p wb-astro --test checks_world`
Expected: FAIL — `unresolved imports wb_astro::mean_stellar_flux, wb_astro::register, ...`.

- [ ] **Step 3: Write `checks/world.rs`**

Create `crates/wb-astro/src/checks/world.rs`:

```rust
//! Checkers over the home world itself: the numbers it states about its own orbit, the
//! temperature those numbers imply, and what the tides will eventually do to its day.

use crate::checks::orbital::checker_model;
use crate::sky::SkyModel;
use crate::units::{AU_M, G, SECONDS_PER_HOUR, STEFAN_BOLTZMANN};
use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};
use wb_constraints::{
    CheckContext, Checker, ConstraintView, Finding, IssueCode, KindPattern, Param, Suggestion,
};
use wb_editlog::{EntityId, Value};

/// The band the climate checker calls habitable, in kelvin (spec §6).
pub const HABITABLE_K: (f64, f64) = (250.0, 320.0);

/// Samples per year for the mean-flux integral. Fixed, like every other count here.
const FLUX_SAMPLES: usize = 36;

/// Mean flux at the home world over one year, W·m⁻², summed over every star.
pub fn mean_stellar_flux(sky: &SkyModel) -> f64 {
    let year = sky.year_days();
    let mut total = 0.0;
    for k in 0..FLUX_SAMPLES {
        let day = year * k as f64 / FLUX_SAMPLES as f64;
        for star in sky.stars() {
            let Some(luminosity) = star.luminosity_w else {
                continue;
            };
            let Some(to_star) = sky.position(star.id, day) else {
                continue;
            };
            let d = to_star.length();
            if d > 0.0 {
                let f = luminosity / (4.0 * PI * d * d);
                if f.is_finite() {
                    total += f;
                }
            }
        }
    }
    total / FLUX_SAMPLES as f64
}

/// `T = ((1 − A)·S / 4σ)^¼`.
pub fn equilibrium_temperature_k(flux: f64, albedo: f64) -> f64 {
    if !flux.is_finite() || flux <= 0.0 {
        return 0.0;
    }
    let absorbed = (1.0 - albedo.clamp(0.0, 1.0)) * flux;
    if absorbed <= 0.0 {
        return 0.0;
    }
    let t = libm::pow(absorbed / (4.0 * STEFAN_BOLTZMANN), 0.25);
    if t.is_finite() { t.max(0.0) } else { 0.0 }
}

/// Tidal-locking timescale for the home world under one perturber, in seconds.
///
/// `t ≈ ω·a⁶·I·Q / (3·G·m²·k₂·R⁵)` with `Q = 100`, `k₂ = 0.3` and `I = 0.33·M·R²` — the
/// usual rigid-body estimate. It gives Earth tens of billions of years under the Moon,
/// which is the right order, and that is all a feasibility hint needs.
pub fn locking_time_s(sky: &SkyModel, perturber_kg: f64, distance_m: f64) -> f64 {
    let home = sky.home();
    let (r, m_home) = (home.radius_m, home.mass_kg);
    if !(perturber_kg > 0.0) || !(distance_m > 0.0) || !(r > 0.0) || !(m_home > 0.0) {
        return f64::INFINITY;
    }
    let omega = TAU / (sky.day_hours() * SECONDS_PER_HOUR);
    let inertia = 0.33 * m_home * r * r;
    let a2 = distance_m * distance_m;
    let a6 = a2 * a2 * a2;
    let r5 = r * r * r * r * r;
    let numerator = omega * a6 * inertia * 100.0;
    let denominator = 3.0 * G * perturber_kg * perturber_kg * 0.3 * r5;
    if denominator <= 0.0 {
        return f64::INFINITY;
    }
    let t = numerator / denominator;
    if t.is_finite() { t } else { f64::INFINITY }
}

fn suggestion(
    code: &str,
    params: Vec<(&str, Param)>,
    writes: Vec<(EntityId, String, Value)>,
) -> Suggestion {
    Suggestion {
        code: IssueCode::new(code),
        params: params
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect::<BTreeMap<String, Param>>(),
        writes,
        creates: Vec::new(),
    }
}

/// A body that states both a period and a distance, inconsistently.
pub struct BodyCheck;

impl Checker for BodyCheck {
    fn id(&self) -> &'static str {
        "astro.body"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("astro.body")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = checker_model(ctx) else {
            return Vec::new();
        };
        let Some(body) = sky.body(c.id) else {
            return Vec::new();
        };
        let (Some(a), Some(p)) = (body.stated_a_m, body.stated_period_s) else {
            return Vec::new();
        };
        let Some(parent) = body.parent.and_then(|id| sky.body(id)) else {
            return Vec::new();
        };
        let implied = crate::kepler::period_s(a, parent.mass_kg);
        if implied <= 0.0 || (p - implied).abs() <= 0.05 * implied {
            return Vec::new();
        }
        let is_moon = body.class == crate::units::BodyClass::Moon;
        let (period_field, distance_field) = if is_moon {
            ("period_days", "distance_km")
        } else {
            ("period_years", "distance_au")
        };
        let unit = if is_moon {
            sky.day_hours() * SECONDS_PER_HOUR
        } else {
            sky.year_days() * sky.day_hours() * SECONDS_PER_HOUR
        };
        let distance_unit = if is_moon { 1_000.0 } else { AU_M };
        vec![
            Finding::issue(self.id(), "astro.period_distance_conflict", 0.7)
                .with_param("stated_period_years", Param::Float(p / unit))
                .with_param("implied_period_years", Param::Float(implied / unit))
                .with_param("stated_distance_au", Param::Float(a / distance_unit))
                .with_suggestion(suggestion(
                    "astro.keep_distance",
                    vec![("distance_au", Param::Float(a / distance_unit))],
                    vec![(body.id, period_field.to_string(), Value::Null)],
                ))
                .with_suggestion(suggestion(
                    "astro.keep_period",
                    vec![("period_years", Param::Float(p / unit))],
                    vec![(body.id, distance_field.to_string(), Value::Null)],
                )),
        ]
    }
}

/// Whether the home world could hold liquid water.
pub struct ClimateCheck;

impl Checker for ClimateCheck {
    fn id(&self) -> &'static str {
        "astro.climate"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("planet")]
    }

    fn check(&self, _c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = checker_model(ctx) else {
            return Vec::new();
        };
        let home = sky.home();
        let flux = mean_stellar_flux(&sky);
        let t = equilibrium_temperature_k(flux, home.albedo);
        if (HABITABLE_K.0..=HABITABLE_K.1).contains(&t) {
            return Vec::new();
        }
        // The year that lands mid-band, from the primary alone: the distance where its
        // flux gives the target temperature, converted back into local days.
        let target = (HABITABLE_K.0 + HABITABLE_K.1) / 2.0;
        let primary = sky.primary_star();
        let mut finding = Finding::issue(self.id(), "astro.outside_habitable_zone", 0.6)
            .with_param("temperature_k", Param::Float(t))
            .with_param("flux_wm2", Param::Float(flux))
            .with_param("albedo", Param::Float(home.albedo));
        if let Some(luminosity) = primary.luminosity_w {
            let absorbed = (1.0 - home.albedo.clamp(0.0, 1.0)).max(f64::MIN_POSITIVE);
            let needed = 4.0 * STEFAN_BOLTZMANN * libm::pow(target, 4.0) / absorbed;
            let distance = (luminosity / (4.0 * PI * needed)).sqrt();
            let period = crate::kepler::period_s(distance, primary.mass_kg);
            let year_days = period / (sky.day_hours() * SECONDS_PER_HOUR);
            if year_days.is_finite() && year_days > 0.0 {
                finding = finding.with_suggestion(suggestion(
                    "astro.retune_year",
                    vec![
                        ("year_days", Param::Float(year_days)),
                        ("distance_au", Param::Float(distance / AU_M)),
                    ],
                    vec![(
                        EntityId::PLANET,
                        "year_days".to_string(),
                        Value::Float(year_days),
                    )],
                ));
            }
        }
        vec![finding]
    }
}

/// Whether the world's day will survive its tides.
pub struct SpinCheck;

impl Checker for SpinCheck {
    fn id(&self) -> &'static str {
        "astro.spin"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("planet")]
    }

    fn check(&self, _c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = checker_model(ctx) else {
            return Vec::new();
        };
        // A civilisation's history: ten thousand local years.
        let budget_s = 10_000.0 * sky.year_days() * sky.day_hours() * SECONDS_PER_HOUR;
        let mut worst: Option<(f64, String)> = None;
        let perturbers = sky
            .moons_of_home()
            .map(|m| (m.mass_kg, m.a_m, m.name.clone()))
            .chain(sky.stars().filter_map(|s| {
                let d = sky.position(s.id, 0.0)?.length();
                Some((s.mass_kg, d, s.name.clone()))
            }));
        for (mass, distance, name) in perturbers {
            let t = locking_time_s(&sky, mass, distance);
            if worst.as_ref().map(|(best, _)| t < *best).unwrap_or(true) {
                worst = Some((t, name));
            }
        }
        let Some((t, name)) = worst else {
            return Vec::new();
        };
        if !(t < budget_s) {
            return Vec::new();
        }
        let years = t / (sky.year_days() * sky.day_hours() * SECONDS_PER_HOUR);
        vec![
            Finding::consequence(self.id(), "astro.tidal_locking")
                .with_param("years", Param::Float(years))
                .with_param("perturber", Param::Text(name)),
        ]
    }
}
```

- [ ] **Step 4: Write `checks/calendar.rs`**

Create `crates/wb-astro/src/checks/calendar.rs`:

```rust
//! Whether the calendar matches the orbit it is supposed to describe.

use crate::checks::orbital::checker_model;
use std::collections::BTreeMap;
use wb_constraints::{
    CheckContext, Checker, ConstraintView, Finding, IssueCode, KindPattern, Param, Suggestion,
};
use wb_editlog::Value;

/// Drift above this many days per year is worth saying out loud.
const DRIFT_TOLERANCE_DAYS: f64 = 0.01;
/// Above half a day, the months simply do not add up.
const SUM_TOLERANCE_DAYS: f64 = 0.5;

/// Months against `year_days`, and the leap rule's residual drift.
pub struct CalendarCheck;

impl Checker for CalendarCheck {
    fn id(&self) -> &'static str {
        "astro.calendar"
    }

    fn kinds(&self) -> &[KindPattern] {
        &[KindPattern::Exact("astro.calendar")]
    }

    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding> {
        let Some(sky) = checker_model(ctx) else {
            return Vec::new();
        };
        let cal = sky.calendar();
        if cal.id != c.id {
            // A second calendar is not the one the model uses; judging it would need a
            // model of its own, and the world has one calendar by design.
            return Vec::new();
        }
        let months = cal.month_days() as f64;
        // Mean calendar year: the months, plus a leap day every n years.
        let mean = months
            + match cal.leap_every_n {
                Some(n) if n >= 2 => 1.0 / n as f64,
                _ => 0.0,
            };
        let year_days = sky.year_days();
        let drift = mean - year_days;
        if drift.abs() > SUM_TOLERANCE_DAYS {
            // Lengthen (or shorten) the longest month to absorb the difference.
            let longest = cal
                .months
                .iter()
                .max_by_key(|m| (m.days, std::cmp::Reverse(m.id)));
            let mut finding = Finding::issue(self.id(), "astro.calendar_sum_mismatch", 0.9)
                .with_param("month_total", Param::Float(months))
                .with_param("year_days", Param::Float(year_days))
                .with_param("difference_days", Param::Float(-drift));
            if let Some(m) = longest {
                let adjusted = m.days - libm::round(drift) as i64;
                if adjusted >= 1 {
                    finding = finding.with_suggestion(Suggestion {
                        code: IssueCode::new("astro.adjust_longest_month"),
                        params: [
                            ("month".to_string(), Param::Text(m.name.clone())),
                            ("days".to_string(), Param::Int(adjusted)),
                        ]
                        .into_iter()
                        .collect::<BTreeMap<String, Param>>(),
                        writes: vec![(m.id, "days".to_string(), Value::Int(adjusted))],
                        creates: Vec::new(),
                    });
                }
                finding = finding.with_related(m.id);
            }
            return vec![finding];
        }
        if drift.abs() > DRIFT_TOLERANCE_DAYS {
            return vec![
                Finding::issue(self.id(), "astro.leap_drift", 0.6)
                    .with_param("drift_days", Param::Float(drift.abs()))
                    .with_param("mean_year_days", Param::Float(mean))
                    .with_param("year_days", Param::Float(year_days)),
            ];
        }
        Vec::new()
    }
}
```

- [ ] **Step 5: Share the model helper and register the four**

In `crates/wb-astro/src/checks/orbital.rs`, make the private `model` helper available to
the sibling modules by renaming it:

```rust
/// Builds the model a checker judges against, or `None` when the system cannot be read.
pub(crate) fn checker_model(ctx: &CheckContext<'_>) -> Option<SkyModel> {
    SkyModel::build(ctx.state).ok()
}
```

and replace the four `model(ctx)` call sites in that file with `checker_model(ctx)`.

Rewrite `crates/wb-astro/src/checks/mod.rs`:

```rust
//! Feasibility checkers (spec §6). Every finding carries the numbers that produced it,
//! and every issue can be kept anyway like any other constraint.

mod calendar;
mod orbital;
mod world;

pub use calendar::CalendarCheck;
pub use orbital::{
    BeltCheck, CompanionCheck, MoonCheck, OrbitCheck, OrderingCheck, critical_distance_m,
    critical_fraction, hill_radius_m, roche_limit_m, simple_ratio,
};
pub use world::{
    BodyCheck, ClimateCheck, HABITABLE_K, SpinCheck, equilibrium_temperature_k, locking_time_s,
    mean_stellar_flux,
};

use wb_constraints::{Checker, CheckerRegistry};

/// Registers all nine astro checkers.
pub fn with_astro_checks(registry: &mut CheckerRegistry) {
    for c in [
        Box::new(BeltCheck) as Box<dyn Checker>,
        Box::new(BodyCheck),
        Box::new(CalendarCheck),
        Box::new(ClimateCheck),
        Box::new(CompanionCheck),
        Box::new(MoonCheck),
        Box::new(OrbitCheck),
        Box::new(OrderingCheck),
        Box::new(SpinCheck),
    ] {
        registry.register(c).expect("astro checker ids are unique");
    }
}
```

- [ ] **Step 6: Add `register` to `lib.rs`**

In `crates/wb-astro/src/lib.rs`, extend the `checks` re-export with the new names and add
the entry point:

```rust
pub use checks::{
    BeltCheck, BodyCheck, CalendarCheck, ClimateCheck, CompanionCheck, HABITABLE_K, MoonCheck,
    OrbitCheck, OrderingCheck, SpinCheck, critical_distance_m, critical_fraction,
    equilibrium_temperature_k, hill_radius_m, locking_time_s, mean_stellar_flux,
    roche_limit_m, simple_ratio, with_astro_checks,
};

use wb_constraints::{CheckerRegistry, KindCatalog};
use wb_editlog::EditLog;

/// Wires this subsystem into a world: its kinds, their validators, and its checkers
/// (spec §5).
pub fn register(log: &mut EditLog, registry: &mut CheckerRegistry, catalog: &mut KindCatalog) {
    register_kinds(log);
    register_catalog(catalog);
    with_astro_checks(registry);
}
```

- [ ] **Step 7: Run the tests**

Run: `cargo test -p wb-astro`
Expected: PASS — every suite, including `checks_world`'s 13 tests.

Watch `a_tidally_locked_day_is_a_consequence_not_an_issue`: the locking estimate spans
dozens of orders of magnitude, so if the close heavy moon does not trip it, print
`locking_time_s` for that moon before adjusting anything — a wrong exponent in `a6` or
`r5` is far more likely than a wrong budget.

- [ ] **Step 8: Run the full gate**

Run: `cargo fmt --all && scripts/check.sh`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/wb-astro
git commit -m "$(cat <<'EOF'
feat(astro): world and calendar checkers, and the register entry point

Four more checkers: a stated period against its own distance, equilibrium
temperature against the 250-320 K band with a year that fixes it, tidal
locking as a consequence, and months against year_days with drift reported
separately from a plain sum mismatch. register() wires kinds, validators and
all nine checkers in one call.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 13: Solar-system validation, Aethoria acceptance, golden hash and perf guard

The evidence that any of this is right. Earth's insolation, daylight and tides are already asserted in Tasks 7 and 8; this task adds the rest of spec §9 — the real solar system, the Aethoria acceptance suite, the cross-target golden hash, and the release-only budget wired into the gate.

**Files:**
- Create: `crates/wb-astro/tests/solar_system.rs`
- Create: `crates/wb-astro/tests/aethoria.rs`
- Create: `crates/wb-astro/tests/golden.rs`
- Create: `crates/wb-astro/tests/perf.rs`
- Modify: `scripts/check.sh`
- Modify: `docs/superpowers/specs/2026-10-18-astronomy-design.md` §9 (record the measured budget)

**Interfaces:**
- Consumes: everything. Produces no new library API.

- [ ] **Step 1: Write the solar-system validation**

Create `crates/wb-astro/tests/solar_system.rs`:

```rust
//! Spec §9: real planetary distances must reproduce real periods, and Neptune and Pluto
//! must come out protected by their resonance rather than crossing.

mod common;

use common::{add, add_gregorian_calendar, astro_report, codes, f, findings, i, named, new_log, set, t};
use wb_astro::SkyModel;
use wb_editlog::{EditLog, EntityId};

/// name, order, class, semi-major axis in AU, eccentricity, real period in years.
const PLANETS: [(&str, i64, &str, f64, f64, f64); 8] = [
    ("Mercury", 1, "rocky", 0.387_098, 0.2056, 0.240_846),
    ("Venus", 2, "rocky", 0.723_332, 0.0068, 0.615_198),
    ("Mars", 4, "rocky", 1.523_68, 0.0934, 1.880_85),
    ("Jupiter", 5, "gas_giant", 5.204_4, 0.0489, 11.862),
    ("Saturn", 6, "gas_giant", 9.582_6, 0.0565, 29.457),
    ("Uranus", 7, "ice_giant", 19.218_4, 0.0457, 84.021),
    ("Neptune", 8, "ice_giant", 30.07, 0.0086, 164.8),
    ("Pluto", 9, "rocky", 39.482, 0.2488, 247.94),
];

fn solar_system() -> EditLog {
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
            ("name", t("Earth")),
            ("order", i(3)),
            ("year_days", f(365.2422)),
            ("day_hours", f(24.0)),
            ("axial_tilt_deg", f(23.44)),
            ("radius_km", f(6_371.0)),
            ("gravity_ms2", f(9.80665)),
        ],
    );
    for (name, order, class, au, e, _) in PLANETS {
        add(
            &mut log,
            "astro.body",
            vec![
                ("name", t(name)),
                ("order", i(order)),
                ("class", t(class)),
                ("distance_au", f(au)),
                ("eccentricity", f(e)),
            ],
        );
    }
    add_gregorian_calendar(&mut log);
    log
}

#[test]
fn real_distances_reproduce_real_periods_within_one_percent() {
    let log = solar_system();
    let sky = SkyModel::build(log.state()).unwrap();
    let year_s = 365.2422 * 24.0 * 3_600.0;
    for (name, _, _, _, _, period_years) in PLANETS {
        let got = sky.body(named(&log, name)).unwrap().period_s / year_s;
        let err = (got - period_years).abs() / period_years;
        assert!(
            err <= 0.01,
            "{name}: derived {got} years against the real {period_years} ({err} off)"
        );
    }
    // And the home world's own orbit, driven the other way round, is 1 AU.
    let err = (sky.home().a_m / wb_astro::AU_M - 1.0).abs();
    assert!(err <= 0.001, "Earth came out at {} AU", sky.home().a_m / wb_astro::AU_M);
}

#[test]
fn neptune_and_pluto_are_a_resonance_not_a_crossing() {
    let log = solar_system();
    let r = astro_report(&log);
    assert!(
        findings(&r, "astro.orbit_crossing").is_empty(),
        "nothing in the real solar system crosses unprotected: {:?}",
        codes(&r)
    );
    let protects = findings(&r, "astro.resonance_protects");
    assert_eq!(protects.len(), 1, "{:?}", codes(&r));
    let (neptune, pluto) = (named(&log, "Neptune"), named(&log, "Pluto"));
    assert!(
        protects[0].related == vec![pluto] || protects[0].related == vec![neptune],
        "the finding should name the other body of the pair"
    );
}

#[test]
fn the_real_solar_system_raises_nothing_else() {
    let r = astro_report(&solar_system());
    let issues: Vec<&str> = r
        .verdicts
        .iter()
        .flat_map(|v| v.issues.iter())
        .map(|f| f.code.as_str())
        .collect();
    assert!(
        issues.is_empty(),
        "the real solar system should be entirely plausible, got {issues:?}"
    );
    assert_eq!(r.counts.implausible, 0);
    assert_eq!(r.counts.stretch, 0);
}
```

- [ ] **Step 2: Write the Aethoria acceptance suite**

Create `crates/wb-astro/tests/aethoria.rs`:

```rust
//! Spec §9 acceptance: the reference world, judged end to end.

mod common;

use common::{aethoria, astro_report, codes, findings, named};
use std::collections::BTreeSet;
use wb_astro::{Date, SkyModel};
use wb_constraints::{IssueCode, keep_anyway};
use wb_grid::LatLon;

#[test]
fn the_system_reads_as_the_lore_describes_it() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    assert_eq!(sky.stars().count(), 2, "two suns");
    assert_eq!(sky.moons_of_home().count(), 3, "three moons");
    assert_eq!(sky.year_days(), 390.0);
    assert_eq!(sky.day_hours(), 30.0);
    assert_eq!(sky.calendar().months.len(), 13);
    assert_eq!(sky.calendar().month_days(), 390);
    assert_eq!(sky.calendar().weekdays.len(), 7, "the Septem");
    assert_eq!(sky.home().order, 5);
    // Thirteen numbered bodies: twelve entities plus the home world.
    let numbered = sky
        .bodies()
        .filter(|b| !b.is_star() && b.a_m > 0.0)
        .count();
    assert_eq!(numbered, 13);
}

#[test]
fn the_three_moons_beat_into_a_repeatable_spring_tide() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    let peak_in = |from: i64| {
        (from..from + 30)
            .map(|d| (sky.tide_range_m(d as f64), d))
            .fold((f64::MIN, from), |best, x| if x.0 > best.0 { x } else { best })
    };
    let (height, day) = peak_in(0);
    assert!(height > 0.0 && height.is_finite());
    // The 10, 15 and 30-day cycles realign every 30 days, every year, for ever.
    for from in [30, 60, 390, 3_900] {
        let (_, d) = peak_in(from);
        assert_eq!(
            (d - day).rem_euclid(30),
            0,
            "the spring tide moved: day {day} then {d}"
        );
    }
    let trough = (0..30)
        .map(|d| sky.tide_range_m(d as f64))
        .fold(f64::MAX, f64::min);
    assert!(
        height > trough * 1.5,
        "three moons should beat hard: {height} against {trough}"
    );
}

#[test]
fn the_calendar_round_trips_across_centuries() {
    let sky = SkyModel::build(aethoria().state()).unwrap();
    for d in [0, 1, 389, 390, 10_000, 390_000, -1, -390_001] {
        assert_eq!(sky.day_index_of(sky.date_of(d)), Some(d));
    }
    // Mehr 1 of year 1 is a Solday, and the year is 13 months of 30.
    assert_eq!(
        sky.date_of(0),
        Date {
            year: 1,
            month_index: 1,
            day: 1,
            weekday_index: 0
        }
    );
    assert_eq!(sky.calendar().weekdays[0], "Solday");
    assert_eq!(sky.date_of(360).month_index, 13);
}

#[test]
fn sol_secundus_is_judged_exactly_as_the_spec_says() {
    let log = aethoria();
    let r = astro_report(&log);
    let secundus = named(&log, "Sol Secundus");

    // It destabilizes the outer system, naming the bodies at risk with real numbers.
    let destab = findings(&r, "astro.companion_destabilizes");
    assert_eq!(destab.len(), 1, "{:?}", codes(&r));
    assert!(!destab[0].related.is_empty());
    assert!(destab[0].related.contains(&named(&log, "Goliath")));
    assert!(
        !destab[0].related.contains(&wb_editlog::EntityId::PLANET),
        "Aethoria itself is inside the critical distance and survives"
    );

    // And "it returns every year" contradicts a 6 AU orbit.
    let mismatch = findings(&r, "astro.companion_period_mismatch");
    assert_eq!(mismatch.len(), 1, "{:?}", codes(&r));
    for key in [
        "stated_period_years",
        "implied_period_years",
        "stated_distance_au",
        "implied_distance_au",
    ] {
        assert!(mismatch[0].params.contains_key(key), "missing {key}");
    }
    // Both findings hang off the companion's own verdict.
    let verdict = r
        .verdicts
        .iter()
        .find(|v| v.constraint == secundus)
        .expect("the companion gets a verdict");
    assert_eq!(verdict.issues.len(), 2);
    assert!(verdict.score < 0.2, "two heavy issues: score {}", verdict.score);
}

#[test]
fn the_resonance_checker_names_a_pair() {
    let log = aethoria();
    let r = astro_report(&log);
    let resonances = findings(&r, "astro.resonance");
    assert!(!resonances.is_empty(), "no resonance found: {:?}", codes(&r));
    // Goliath and Stormheart are a 2:1, as the lore's "orbital resonance" implies. The
    // finding is raised from Goliath, the lower id, so the ratio is stated as its own
    // period over Stormheart's: 1:2.
    let pair = resonances
        .iter()
        .find(|f| f.related.contains(&named(&log, "Stormheart")))
        .expect("Goliath and Stormheart");
    assert!(matches!(pair.params.get("p"), Some(wb_constraints::Param::Int(1))));
    assert!(matches!(pair.params.get("q"), Some(wb_constraints::Param::Int(2))));
}

#[test]
fn the_worlds_verdicts_are_exactly_these() {
    // The whole point of the engine: a world-builder can see every judgement at once.
    let log = aethoria();
    let r = astro_report(&log);
    let issues: BTreeSet<&str> = r
        .verdicts
        .iter()
        .flat_map(|v| v.issues.iter())
        .map(|f| f.code.as_str())
        .collect();
    let expected: BTreeSet<&str> = [
        // Sol Secundus, as above.
        "astro.companion_destabilizes",
        "astro.companion_period_mismatch",
        // Goliath shares its number with Sol Secundus, so one of them is out of sequence.
        "astro.order_mismatch",
        // Two crossings, both from the lore: Voidwalker's highly elliptical orbit reaches
        // inside Rimefrost's, and Goliath "shares orbit with Sol Secundus".
        "astro.orbit_crossing",
    ]
    .into_iter()
    .collect();
    assert_eq!(issues, expected, "{:?}", codes(&r));
    assert!(findings(&r, "astro.calendar_sum_mismatch").is_empty());
    assert!(findings(&r, "astro.outside_habitable_zone").is_empty());
    assert!(findings(&r, "astro.tidal_locking").is_empty());
}

#[test]
fn keeping_the_lore_anyway_marks_it_intentional() {
    let mut log = aethoria();
    let secundus = named(&log, "Sol Secundus");
    keep_anyway(
        &mut log,
        secundus,
        &[
            IssueCode::new("astro.companion_destabilizes"),
            IssueCode::new("astro.companion_period_mismatch"),
        ],
        Some("Sol Secundus is held in place by the Weave"),
    )
    .unwrap();
    let r = astro_report(&log);
    let verdict = r
        .verdicts
        .iter()
        .find(|v| v.constraint == secundus)
        .expect("a verdict");
    assert!(verdict.issues.is_empty());
    assert_eq!(verdict.intentional.len(), 2);
    assert_eq!(verdict.status, wb_constraints::Status::Intentional);
    assert_eq!(verdict.score, 1.0);
}

#[test]
fn the_sky_is_legible_all_year() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let lat = LatLon::from_degrees(35.0, 0.0);
    for d in 0..390 {
        let day = d as f64;
        assert!(sky.insolation(lat, day) > 0.0);
        let hours = sky.daylight_hours(lat, day);
        assert!((0.0..=30.0).contains(&hours));
        assert!(sky.tide_range_m(day) >= 0.0);
        for b in sky.bodies() {
            if b.is_home {
                continue;
            }
            assert!(sky.position(b.id, day).is_some());
            if let Some(p) = sky.phase(b.id, day) {
                assert!((0.0..=1.0).contains(&p));
            }
        }
    }
    // Dual Light: the two suns are both up for part of the year, so the companion's
    // contribution peaks well above its annual mean.
    let companion = named(&log, "Sol Secundus");
    let distances: Vec<f64> = (0..390)
        .map(|d| sky.position(companion, d as f64).unwrap().length())
        .collect();
    let nearest = distances.iter().copied().fold(f64::MAX, f64::min);
    let farthest = distances.iter().copied().fold(0.0_f64, f64::max);
    assert!(
        farthest > nearest * 1.3,
        "the companion should approach and recede: {nearest} to {farthest}"
    );
}
```

- [ ] **Step 3: Run both validation suites**

Run: `cargo test -p wb-astro --test solar_system --test aethoria`
Expected: PASS.

`the_worlds_verdicts_are_exactly_these` is the suite's spine and the most likely to need
adjusting: if the set differs, do **not** loosen it to whatever came out. Read each extra
or missing code against the fixture and decide which is wrong — the fixture's distances
are this engine's reading of the lore and may legitimately need a nudge, but a verdict
nobody can explain is a bug in a checker.

- [ ] **Step 4: Write the cross-target golden**

Create `crates/wb-astro/tests/golden.rs`:

```rust
//! Cross-target determinism gate. Every number `SkyModel` computes for a fixed system is
//! hashed bit-for-bit; the same hash must come out on aarch64, wasm32-wasip1 and
//! x86_64-musl (`scripts/ci-local.sh` runs all three). Update GOLDEN_SKY in the same
//! commit as any intentional change to the physics.

mod common;

use common::aethoria;
use wb_astro::SkyModel;
use wb_grid::LatLon;
use wb_world::to_hex;

const GOLDEN_SKY: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[test]
fn golden_sky() {
    let log = aethoria();
    let sky = SkyModel::build(log.state()).unwrap();
    let mut h = blake3::Hasher::new();
    h.update(b"wb-astro-golden-v1");
    let mut bits = |x: f64| h.update(&x.to_bits().to_le_bytes());

    // Derived values for every body, in EntityId order.
    for b in sky.bodies() {
        h.update(b.name.as_bytes());
        bits(b.a_m);
        bits(b.period_s);
        bits(b.mass_kg);
        bits(b.radius_m);
        bits(b.density_kgm3);
        bits(b.albedo);
        bits(b.luminosity_w.unwrap_or(0.0));
        h.update(&b.order.to_le_bytes());
    }
    // Outputs through the year, at a stride that is coprime with every moon cycle.
    for k in 0..30 {
        let day = k as f64 * 13.0;
        bits(sky.tide_range_m(day));
        for lat_deg in [-85.0, -45.0, 0.0, 23.5, 45.0, 85.0] {
            let lat = LatLon::from_degrees(lat_deg, 0.0);
            bits(sky.insolation(lat, day));
            bits(sky.daylight_hours(lat, day));
        }
        for b in sky.bodies() {
            if let Some(p) = sky.position(b.id, day) {
                bits(p.x);
                bits(p.y);
                bits(p.z);
            }
            bits(sky.phase(b.id, day).unwrap_or(-1.0));
            bits(sky.apparent_brightness(b.id, day).unwrap_or(0.0));
        }
    }
    // Calendar arithmetic, both sides of the epoch.
    for d in [-390_001i64, -1, 0, 1, 389, 390, 1_000_000] {
        let date = sky.date_of(d);
        h.update(&date.year.to_le_bytes());
        h.update(&date.month_index.to_le_bytes());
        h.update(&date.day.to_le_bytes());
        h.update(&(date.weekday_index as u64).to_le_bytes());
    }
    // And the proposed seasons, which read the whole sunlight curve.
    for s in sky.propose_seasons(4) {
        h.update(s.code.as_str().as_bytes());
        for e in &s.creates {
            h.update(e.kind.as_bytes());
            for (f, v) in &e.fields {
                h.update(f.as_bytes());
                h.update(&postcard_bytes(v));
            }
        }
    }
    let got = to_hex(h.finalize().as_bytes());
    assert_eq!(
        got, GOLDEN_SKY,
        "sky values changed; if intentional, set GOLDEN_SKY = \"{got}\""
    );
}

fn postcard_bytes(v: &wb_editlog::Value) -> Vec<u8> {
    // The Value encoding is already pinned by wb-editlog's own golden test.
    postcard::to_allocvec(v).expect("values always serialize")
}
```

`postcard` is not a dependency of `wb-astro`. Rather than adding one for a test, hash the
`Value`s by hand — replace `postcard_bytes` and its call with a small match that writes
the `Text`, `Int` and `Entity` cases the season fields actually use, and delete the
`postcard` reference:

```rust
fn value_bytes(h: &mut blake3::Hasher, v: &wb_editlog::Value) {
    match v {
        wb_editlog::Value::Text(s) => {
            h.update(b"t");
            h.update(s.as_bytes());
        }
        wb_editlog::Value::Int(n) => {
            h.update(b"i");
            h.update(&n.to_le_bytes());
        }
        wb_editlog::Value::Entity(e) => {
            h.update(b"e");
            h.update(&e.op.lamport.to_le_bytes());
            h.update(&e.n.to_le_bytes());
        }
        other => {
            h.update(b"?");
            h.update(format!("{other:?}").as_bytes());
        }
    }
}
```

- [ ] **Step 5: Set the golden hash**

Run: `cargo test -p wb-astro --test golden`
Expected: FAIL once, printing the real hash. Set `GOLDEN_SKY` to it.

Run: `cargo test -p wb-astro --test golden && cargo test -p wb-astro --test golden --target wasm32-wasip1`
Expected: PASS on both. If wasm disagrees with native, stop: that is exactly the bug this
test exists to catch, and the cause is a platform transcendental or an FMA that clippy's
list does not cover, not a reason to keep two hashes.

- [ ] **Step 6: Write the perf guard**

Create `crates/wb-astro/tests/perf.rs`:

```rust
//! Release-only budget (spec §9): a thirteen-body system with twenty moons computes a
//! full year of daily insolation and tides within budget.

mod common;

use common::{add, aethoria, f, i, t};
use wb_astro::SkyModel;
use wb_editlog::{EditLog, EntityId, Value};
use wb_grid::LatLon;

/// Aethoria plus seventeen more moons, spread over the gas giants and the home world.
fn heavy_system() -> EditLog {
    let mut log = aethoria();
    let hosts: Vec<EntityId> = log
        .state()
        .live()
        .filter(|e| {
            e.kind() == Some("astro.body")
                && matches!(e.get("class"), Some(Value::Text(c)) if c == "gas_giant")
        })
        .map(|e| e.id)
        .collect();
    let mut n = 0;
    for host in hosts {
        for k in 1..=5 {
            n += 1;
            add(
                &mut log,
                "astro.body",
                vec![
                    ("name", t(&format!("Moonlet {n}"))),
                    ("order", i(k)),
                    ("class", t("moon")),
                    ("parent", Value::Entity(host)),
                    ("period_days", f(3.0 * k as f64)),
                ],
            );
        }
    }
    for k in 1..=2 {
        n += 1;
        add(
            &mut log,
            "astro.body",
            vec![
                ("name", t(&format!("Moonlet {n}"))),
                ("order", i(3 + k)),
                ("class", t("moon")),
                ("parent", Value::Entity(EntityId::PLANET)),
                ("period_days", f(45.0 + 7.0 * k as f64)),
            ],
        );
    }
    log
}

#[test]
#[cfg_attr(debug_assertions, ignore = "release-only performance guard")]
fn a_year_of_daily_sky_is_within_budget() {
    let log = heavy_system();
    let sky = SkyModel::build(log.state()).unwrap();
    assert_eq!(sky.moons_of_home().count(), 5);
    assert!(sky.bodies().count() >= 33, "{}", sky.bodies().count());

    let lat = LatLon::from_degrees(45.0, 0.0);
    let start = std::time::Instant::now();
    let mut total = 0.0;
    for d in 0..390 {
        let day = d as f64;
        total += sky.insolation(lat, day);
        total += sky.tide_range_m(day);
    }
    let ms = start.elapsed().as_millis();
    assert!(total > 0.0);
    // Measured on the development Mac: <FILL IN> ms native, <FILL IN> ms wasm. The budget
    // is roughly double, so a loaded machine does not flake it — a guard that flakes gets
    // disabled, which is worse than a loose one.
    let budget = if cfg!(target_family = "wasm") { 3_000 } else { 1_500 };
    assert!(
        ms <= budget,
        "a year of daily sky for 33 bodies took {ms} ms (budget {budget} ms)"
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "release-only performance guard")]
fn evaluating_the_whole_system_is_within_budget() {
    let log = heavy_system();
    let start = std::time::Instant::now();
    let report = common::astro_report(&log);
    let ms = start.elapsed().as_millis();
    assert!(report.verdicts.len() >= 33);
    // Every astro checker rebuilds the model per entity, which is the cost this measures.
    // Measured: <FILL IN> ms native, <FILL IN> ms wasm.
    let budget = if cfg!(target_family = "wasm") { 4_000 } else { 2_000 };
    assert!(
        ms <= budget,
        "evaluated {} verdicts in {ms} ms (budget {budget} ms)",
        report.verdicts.len()
    );
}
```

- [ ] **Step 7: Measure and set the budgets**

Run: `cargo test --release -p wb-astro --test perf -- --nocapture`
Run: `cargo test --release -p wb-astro --test perf --target wasm32-wasip1 -- --nocapture`

Both must pass. Replace each `<FILL IN>` with the measured milliseconds, and if a
measurement exceeds half its budget, raise that budget to roughly double the measurement
rather than leaving it tight. If a measurement exceeds the budget outright, report the
number and the per-entity rebuild cost before changing anything: the recorded fix is the
`CheckContext` memo slot, not a cache inside a checker.

- [ ] **Step 8: Wire the guard into the gate**

In `scripts/check.sh`, after the `wb-constraints` perf lines, add:

```bash
cargo test --release -p wb-astro --test perf
cargo test --release -p wb-astro --test perf --target wasm32-wasip1
```

- [ ] **Step 9: Record the measured budget in the spec**

In `docs/superpowers/specs/2026-10-18-astronomy-design.md` §9, replace:

```markdown
- **Performance guard:** a 13-body system with 20 moons computes a full year of daily insolation and tides within a budget set from the first measurement, release-only, wired into `scripts/check.sh`.
```

with the real numbers:

```markdown
- **Performance guard:** a 13-body system with 20 moons computes a full year of daily
  insolation and tides in <MEASURED> ms native and <MEASURED> ms wasm, against budgets of
  <BUDGET> and <BUDGET> ms; evaluating every checker over the same system takes
  <MEASURED>/<MEASURED> ms against <BUDGET>/<BUDGET>. Release-only, wired into
  `scripts/check.sh`.
```

- [ ] **Step 10: Run the full gate, then the cross-target gate**

Run: `cargo fmt --all && scripts/check.sh`
Expected: PASS.

Run: `scripts/ci-local.sh`
Expected: PASS — including the musl cross-compile and the amd64 Alpine container run, which
is where `golden_sky` proves itself on a third architecture.

- [ ] **Step 11: Commit**

```bash
git add crates/wb-astro scripts/check.sh docs/superpowers/specs/2026-10-18-astronomy-design.md
git commit -m "$(cat <<'EOF'
test(astro): solar-system validation, Aethoria acceptance, golden and perf

Real planetary distances reproduce real periods within 1% and Neptune and
Pluto come out protected by their 3:2 rather than crossing. Aethoria's
acceptance suite pins the whole reference world: two suns, three moons beating
on a repeatable 30-day spring tide, a round-tripping 13-month calendar, and
Sol Secundus judged with real numbers on both counts. golden_sky hashes every
computed value bit-for-bit across all three targets, and the release-only perf
guard is wired into scripts/check.sh.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

## Self-Review

Checked against the spec, section by section.

**Spec coverage.** §3.1 bodies → Tasks 5–6; §3.2 stars → 5–6; §3.3 home world → 6 (with the gravity amendment); §3.4 moons → 5–6; §3.5 class defaults → 3; §3.6 calendar → 6, 9; §3.7 nothing derived is stored → 6 (`SkyModel` owns every derived value; no task writes one back); §3.8 `KindCatalog` → 1; §4.1 orbits → 4; §4.2 sunlight → 7; §4.3 tides → 8; §4.4 positions, phases, brightness → 6, 8; §4.5 calendar and `propose_seasons` → 9, 10; §5 API → every method appears in a task's Interfaces block, with `register` in 12; §6 all thirteen codes → 11 (nine) and 12 (five, counting both calendar codes); §7 `AstroError` → 3, raised in 6; §8 determinism → the Global Constraints, enforced by `clippy.toml`, and proved by Task 13's golden; §9 testing → Earth validation in 7–8, the rest in 13; §10 out of scope → nothing in this plan draws the sky, catalogues events, perturbs N bodies, edits raw elements or overrides mechanics.

**Placeholders.** Two remain by design, both in Task 13 Step 7: the perf budgets' `<FILL
IN>`, which exist precisely because a budget must come from a measurement on the machine
that runs it, and `GOLDEN_SKY`'s zeros, which the first run replaces. Both steps say
exactly how to resolve them. Everywhere else, every code step carries the code.

**Type consistency.** `SkyModel`'s methods keep one spelling throughout
(`insolation`, `daylight_hours`, `tide_range_m`, `position`, `heliocentric`, `phase`,
`apparent_brightness`, `date_of`, `day_index_of`, `season_of`, `propose_seasons`,
`bodies`, `body`, `home`, `primary_star`, `stars`, `moons_of_home`, `calendar`,
`year_days`, `day_hours`, `axial_tilt_rad`, `is_retrograde`, `seconds_of`,
`declination`). `BodyView`'s field names are fixed in Task 6 and used unchanged in 7–13.
`CalendarModel::days_in` is defined in Task 6 and **replaced** in Task 9 — the only
deliberate redefinition, called out in both tasks. `checker_model` is introduced as
private `model` in Task 11 and renamed in Task 12 Step 5, which that step spells out.
`with_astro_checks` registers five checkers in Task 11 and nine in Task 12, stated in
both Interfaces blocks.
