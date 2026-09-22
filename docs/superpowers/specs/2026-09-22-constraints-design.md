# Constraints & Feasibility — Subsystem [3] Design

- **Date:** 2026-09-22
- **Status:** Approved in brainstorming; awaiting written-spec review
- **Parent spec:** `docs/superpowers/specs/2026-09-21-world-builder-architecture-design.md` (§2.3 invariants, §5 Constraint & Feasibility System)
- **Depends on:** `wb-grid`, `wb-world` (geometry, `Planet`, `polygon_area_m2`), `wb-editlog` (state, transactions, validators)
- **Crate:** `crates/wb-constraints`

---

## 1. Purpose

Turn user-created entities into typed constraints, run registered checkers over them, and produce one explainable verdict per constraint — graded against the realism dial, honouring "keep anyway" overrides, and offering one-click suggestions that apply as ordinary Edit Log transactions. This phase delivers the framework plus a starter set of checks that need no simulation; later subsystems (pipeline stages, Civilization, Codex) register additional kinds and checkers through the same interfaces.

## 2. Decisions (from brainstorming)

| Topic | Decision |
|---|---|
| World score | No world-level score in v1 — per-constraint verdicts and counts only. Per-constraint scores are kept so an opt-in world score can be added later. |
| Scope | Framework + starter static checks (no simulation). Bulk Aethoria lore-fact import belongs to Codex [16] around v1.5–2.0. |
| Architecture | Registry of pure `Checker`s. A declarative rule language (customer-written rules, tied to Custom Fields [14]) is a later extension of the same registry. |
| Verdict status | `Open` or `Intentional`. The parent spec's "adjusted" status is dropped: whether a suggestion was applied is visible in the Edit Log timeline. |

## 3. Constraints

A **constraint** is any live Edit Log entity whose `kind` is registered in the constraint kind catalog (§6). Its `ConstraintId` is its `EntityId`.

Common optional fields on every constraint kind:

| Field | Type | Meaning |
|---|---|---|
| `name` | Text | Display name |
| `strength` | Text `"target"` \| `"hard"` | Default `"target"`. Consumed by later stages (hard = must hold exactly); grading treats both the same. |
| `tolerance` | Float in [−1, 1] | Per-constraint shift of the realism slider |
| `keep_codes` | List of Text | Issue codes the user chose to keep anyway |
| `keep_reason` | Text | Optional lore reason ("Raised in the Titan War") |

## 4. Findings and verdicts

### 4.1 Findings

```
Finding {
  checker:     String            // checker id, e.g. "static.lore_area"
  kind:        Support | Issue | Consequence
  code:        IssueCode         // dotted string, e.g. "lore.area_mismatch"
  badness:     f64 in [0, 1]     // Issue only; 0 for Support/Consequence
  params:      BTreeMap<String, Param>   // Param = Int | Float | Text | Entity
  related:     Vec<EntityId>
  suggestions: Vec<Suggestion>
}
```

- **Support** explains why a constraint works; never lowers the grade.
- **Issue** is a problem; its `badness` drives the score.
- **Consequence** is informational (knock-on effects).

### 4.2 Score and grade

- **Score** = `1 − max(badness of issues whose code is not in keep_codes)`; `1.0` when there are none.
- **Effective realism** `r = clamp(slider + tolerance, 0, 1)`.
- **Grade:**
  - ✅ `Plausible` if `score ≥ 0.8 − 0.3·r`
  - ❌ `Implausible` if `score < 0.4 − 0.3·r`
  - ⚠️ `Stretch` otherwise
- **Presets:** `EarthStrict` = 0.0, `PlausibleFantasy` = 0.5, `HighFantasy` = 0.85. The slider is stored on the planet entity as `realism` (Float); absent means `PlausibleFantasy`.
- **Status:** `Intentional` if the constraint has at least one issue and every issue's code is kept; otherwise `Open`. Kept issues are listed separately as intentional.
- Keeping is **by code**: an issue code that appears later (after the constraint is edited) and is not in `keep_codes` counts normally.
- All arithmetic is plain `f64` (+, −, ×, comparisons) — deterministic on every target.

### 4.3 Verdict and report

```
Verdict { constraint, kind, grade, score, status, issues, intentional, supports, consequences }
Report  { verdicts (EntityId order), counts { plausible, stretch, implausible, intentional }, hash }
```

`Report::hash` = `blake3("wb-report-v1" ‖ postcard(verdicts))` — the cross-target determinism check. There is no world-level score in v1.

## 5. Checkers and evaluation

### 5.1 Interface

```rust
pub trait Checker: Send + Sync {
    fn id(&self) -> &'static str;
    fn kinds(&self) -> &[KindPattern];       // Exact("lore.area") | Prefix("feature.")
    fn check(&self, c: ConstraintView<'_>, ctx: &CheckContext<'_>) -> Vec<Finding>;
}
```

Checkers are pure and deterministic: same inputs → same findings; no log writes, no randomness, no platform math. Missing or malformed inputs yield no finding (never a panic or error).

### 5.2 Registry

`CheckerRegistry` holds checkers keyed by id (sorted). Registering a duplicate id returns `ConstraintError::DuplicateChecker`. `CheckerRegistry::with_starter_checks()` returns the six starter checkers (§7).

### 5.3 Context

```rust
pub struct CheckContext<'a> {
    pub state: &'a wb_editlog::State,
    pub planet: wb_world::Planet,
    pub realism: f64,
    pub terrain: &'a dyn Terrain,
}
pub trait Terrain: Send + Sync { fn is_land(&self, p: LatLon) -> Option<bool>; }
pub struct UnknownTerrain;   // always None
```

Checkers that need terrain produce nothing when it answers `None`. Later subsystems add further optional context (elevation, precipitation) without breaking existing checkers.

### 5.4 Evaluation

`evaluate(state, ctx, registry) -> Report`:
1. Collect live constraints (registered kinds, not deleted) in `EntityId` order.
2. For each, run matching checkers in id order; collect findings.
3. Sort findings by `(checker, code, params)`; clamp `badness` into [0, 1].
4. Apply keep-overrides and grading (§4.2); build verdicts, counts, and hash.

v1 re-evaluates everything each call; incremental re-checking is deferred until simulation-backed checkers exist.

### 5.5 Explanations

A template catalog maps `IssueCode` → English text with `{param}` placeholders (`render(&Finding) -> String`). Missing templates fall back to `"<code> <params>"`. Localization and map pins reuse the same data.

## 6. Kind catalog (starter)

Registered via `register_kinds(&mut EditLog)` (validators) and `is_constraint_kind(&str)`.

| Kind | Required | Optional |
|---|---|---|
| `feature.mountain_range`, `feature.hills` | `spine` (LineString) or `area` (Polygon) | `peak_m` (Float) |
| `feature.forest` | `area` (Polygon) | `biome`: `tropical` \| `temperate` \| `boreal` |
| `feature.desert`, `feature.lake` | `area` (Polygon) | — |
| `feature.glacier` | `area` (Polygon) | `elevation_m` (Float) |
| `feature.river` | `path` (LineString, source → mouth, ≥ 2 points) | — |
| `civ.settlement` | `at` (LatLon), `name` (Text) | `role` (Text) |
| `rule.region` | `area` (Polygon), `rule`: `no_rain` \| `permanent_ice` \| `perpetual_storm` | — |
| `lore.area` | `subject` (Entity), `value` (Float > 0), `unit`: `km2` \| `mi2` | — |
| `lore.relative_position` | `subject`, `object` (Entity), `relation`: `north_of` \| `south_of` \| `east_of` \| `west_of` | — |

Validators check types and enumerations only; semantic problems are findings, not validation failures.

**Edit Log extension:** `wb-editlog` gains `Value::Entity(EntityId)` as the last `Value` variant (existing encodings and goldens unchanged). A reference to a missing entity is not a validation error; checkers skip it.

## 7. Starter checkers

| Id | Kinds | Finding |
|---|---|---|
| `static.coast` | `feature.*` except `feature.lake`, `feature.river`, `feature.glacier`; `civ.settlement` | Sample the geometry (LatLon; every LineString/Polygon vertex plus segment midpoints). Issue `coast.over_ocean`, badness = fraction of samples where `is_land == Some(false)`, only if > 0. Skipped if any sample is `None`. |
| `static.lore_area` | `lore.area` | Subject must have a Polygon `area`. `ratio = max(drawn, stated) / min(drawn, stated)` (km² from `polygon_area_m2`; mi² × 2.589988110336). Badness 0 if ratio ≤ 1.15, else `min(1, (ratio − 1.15) / (3 − 1.15))`. Issue `lore.area_mismatch` with params `drawn`, `stated`, `unit`, `ratio`; suggestion: set `value` to the drawn area in the stated unit. |
| `static.relative_position` | `lore.relative_position` | Centroid (mean of unit vectors of all vertices, re-projected) of subject and object. North/south compare latitude; east/west compare the signed longitude difference wrapped to (−π, π]. Violation magnitude `d` in degrees; badness `min(1, 0.2 + d / 20)` when violated. Issue `lore.relative_position_violated`. |
| `static.region_rules` | `rule.region` | For each live feature whose centroid lies inside the rule polygon and whose kind the rule forbids (`no_rain` → lake, river, forest; `permanent_ice` → forest, desert, settlement; `perpetual_storm` → none): issue `rule.forbidden_feature`, badness 0.6, `related` = that feature. |
| `static.river_mouth` | `feature.river` | Needs terrain. Mouth on land and no `feature.lake` polygon containing it → `river.inland_mouth`, badness 0.3. Source on ocean → `river.source_in_ocean`, badness 0.8. |
| `static.latitude_sanity` | `feature.glacier`, `feature.forest` | Glacier with `elevation_m < 500` (absent = 0) and centroid within ±25° → `climate.lowland_tropical_glacier`, 0.6. Tropical forest centroid beyond ±35° → `climate.tropical_forest_high_latitude`, 0.5. Boreal forest within ±30° → `climate.boreal_forest_low_latitude`, 0.4. Placeholders to be superseded by the climate stage. |

Point-in-polygon on the sphere uses the winding test: sum, over polygon edges (a, b), the signed angle `atan2((a × b) · p, (a · b) − (a · p)(b · p))`-style turn as seen from point `p` (implemented with `libm::atan2` on unit vectors); the point is inside when the absolute sum exceeds π. Polygons must be smaller than a hemisphere (as in Phase 1). Segment midpoints are great-circle midpoints `normalize(a + b)`. The plan pins the exact formula with unit tests, including a polygon straddling the antimeridian and one containing a pole.

## 8. Actions

- `apply_suggestion(&mut EditLog, &Suggestion) -> Result<OpId, ConstraintError>` — one transaction labelled from the suggestion's template; fails with `UnknownEntity` (no change) if any target entity no longer exists.
- `keep_anyway(&mut EditLog, constraint, codes, reason) -> Result<OpId, ConstraintError>` — union into `keep_codes`, set `keep_reason` if given; label `"Kept anyway: <name or kind>"`.
- `unkeep(&mut EditLog, constraint, codes) -> Result<OpId, ConstraintError>` — remove codes; clears `keep_reason` when none remain.
- `set_realism(&mut EditLog, Preset | f64) -> Result<OpId, ConstraintError>` — writes `realism` on the planet (clamped to [0, 1]).

All are ordinary, undoable Edit Log transactions.

## 9. Errors

`ConstraintError`: `DuplicateChecker(String)`, `UnknownEntity(EntityId)`, `NotAConstraint(EntityId)`, `Edit(EditError)`. Checkers never error.

## 10. Testing

- **Grading properties:** score monotone in issue badness; grade never harsher as `r` rises; kept codes excluded while new codes count; presets map to thresholds; `tolerance` clamps.
- **Per-checker unit tests** with a fake terrain (land where longitude < 0) and hand-built states.
- **Aethoria cases:** Great Sand Sea lore 600 000 mi² vs drawn ≈ 1.2 M mi² → Stretch/Implausible by preset; suggestion fixes it; undo restores. "Thulean Mountains north of the Sand Sea" → Plausible when drawn so, Implausible when swapped.
- **End-to-end:** Edit Log world → evaluate → apply suggestion → keep anyway → undo → re-evaluate; grades checked at each step.
- **Golden:** a fixed world's `Report::hash` identical on aarch64, wasm32-wasip1, x86-64 (via `scripts/ci-local.sh`).
- **Performance guard:** 5 000 constraints evaluated in < 300 ms (native release) and < 600 ms (wasm32-wasip1 release), release-only test wired into `scripts/check.sh`. (Measured ≈190 ms native and ≈227 ms wasm on the development Mac after the bounding-box prefilter; the budgets leave headroom for a loaded machine. The earlier 200 ms figure was set before the per-constraint scanning cost was measured, against a fixture of identical deserts and unknown terrain that never exercised the region-rule or coast scans.)

## 11. Out of scope

Simulation-backed checkers (pipeline stages), the declarative rule language (Custom Fields [14]), bulk lore import (Codex [16], v1.5–2.0), world-level score (later, opt-in), report UI and map pins (Editor [10]), incremental re-checking.
