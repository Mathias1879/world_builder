# Astronomy & Calendar — Subsystem [13] Design

- **Date:** 2026-10-18
- **Status:** Approved in brainstorming; awaiting written-spec review
- **Parent spec:** `docs/superpowers/specs/2026-09-21-world-builder-architecture-design.md` (§2.3 invariants, §7B Astronomy & Calendar)
- **Depends on:** `wb-grid` (`LatLon`), `wb-world` (`Planet`), `wb-editlog` (state, entities, validators), `wb-constraints` (checker registry, verdicts, suggestions)
- **Crate:** `crates/wb-astro`

---

## 1. Purpose

Model a whole planetary system — several stars, planets, moons, belts — from friendly inputs, and derive the numbers the rest of the engine needs: sunlight at any latitude on any day (summed over every star), daylight length, tides from every moon, body positions and brightness, and a calendar that converts between day numbers and dates. Register feasibility checkers so the star system is judged like every other constraint, with explanations, suggestions and "keep anyway".

## 2. Decisions (from brainstorming)

| Topic | Decision |
|---|---|
| v1 scope | Numbers + calendar + feasibility. Positions and brightness are computed (they fall out of the orbits) but nothing draws them yet. The eclipse/conjunction **event catalogue** is designed for and deferred. |
| Inputs | Friendly: periods, cycles, orbital order. Physics derived and exposed read-only. Direct editing of orbital elements is a later update. |
| Seasons | User-defined named date ranges of any number and length, **plus** `propose_seasons(n)` suggestions read off the sunlight curve. |
| Model | Classical two-body orbits solved analytically (Kepler). No N-body integration. Stability judged by published analytic criteria. |
| System shape | A full system: numbered bodies, sibling planets with their own moons, belts, ring systems, and stars that may themselves orbit inside the system. |

## 3. Data model

### 3.1 Bodies

One kind describes every object. `astro.body`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `name` | Text | yes | "Pyrox", "The Veil", "Lumara's Tear" |
| `order` | Int ≥ 1 | yes | Position outward from the parent (1-based) |
| `class` | Text | yes | `rocky` \| `water` \| `gas_giant` \| `ice_giant` \| `belt` \| `moon` |
| `parent` | Entity | no | The body it orbits. Absent = the primary star. |
| `period_years` | Float > 0 | one of | Orbital period (`period_days` for moons, see §3.4) |
| `distance_au` | Float > 0 | one of | Semi-major axis (`distance_km` for moons) |
| `eccentricity` | Float 0–0.95 | no | Default 0 |
| `inclination_deg` | Float −90–90 | no | Default 0 |
| `radius_km` | Float > 0 | no | Default by class (§3.5) |
| `density_kgm3` | Float > 0 | no | Default by class |
| `albedo` | Float 0–1 | no | Default by class; drives apparent brightness |
| `rings` | Bool | no | Display and lore only in v1 |
| `inner_au` / `outer_au` | Float > 0 | belt only | A belt's extent; replaces `distance_au` |

Exactly one of `period_years` / `distance_au` is required; giving both is a feasibility finding, not an error (§6).

### 3.2 Stars

`astro.star`:

| Field | Type | Required | Meaning |
|---|---|---|---|
| `name` | Text | yes | "Sol Prime" |
| `role` | Text | yes | `primary` \| `companion` |
| `mass_solar` | Float > 0 | yes | Mass in solar masses |
| `luminosity_solar` | Float > 0 | no | Derived from mass when absent: `L = M^3.5` |
| `parent` | Entity | no | For a companion: the body or star it orbits (absent = the primary) |
| `order`, `period_years`, `distance_au`, `eccentricity`, `inclination_deg` | — | no | Same meaning as a body; a companion star placed inside the planetary system uses these |

Exactly one `role = primary` star must exist; it is the origin of the system.

### 3.3 The home world

The home world is the **existing planet entity** (`EntityId::PLANET`), not a separate body. It gains:

| Field | Type | Meaning |
|---|---|---|
| `order` | Int | Its position in the system (5 for Aethoria) |
| `year_days` | Float > 0 | Days per orbit, in local days |
| `day_hours` | Float > 0 | Length of a local day in hours |
| `axial_tilt_deg` | Float 0–90 | Obliquity |
| `eccentricity` | Float 0–0.95 | Default 0 |
| `retrograde` | Bool | Spin direction; default false |
| `radius_km` | Float > 0 | Planet radius; default 6371 (Earth) |
| `gravity_ms2` | Float > 0 | Surface gravity; default 9.80665 (Earth) |

`wb_world::Planet` carries a radius for the grid, but the home world's *physical* radius
and gravity are world data, so they live on the planet entity like every other editable
value, with Earth's figures as defaults. Mass follows as `M = g·R²/G`.

### 3.4 Moons

A moon is `astro.body` with `class = moon` and `parent` = a planet. It uses `period_days` and `distance_km` in place of the year/AU fields. Only moons whose parent is the home world contribute to tides; the rest are sky and lore.

### 3.5 Class defaults

| Class | radius_km | density_kgm3 | albedo |
|---|---|---|---|
| `rocky` | 5 000 | 5 000 | 0.15 |
| `water` | 6 500 | 3 000 | 0.30 |
| `gas_giant` | 60 000 | 1 300 | 0.50 |
| `ice_giant` | 25 000 | 1 600 | 0.50 |
| `moon` | 1 500 | 3 300 | 0.12 |
| `belt` | — | — | 0.10 |

Defaults exist so a world-builder can write "a gas giant" and get sensible numbers; any field can be set explicitly.

### 3.6 Calendar

- `astro.calendar` (one per world): `epoch_name` (Text, e.g. "AE"), `weekdays` (List of Text, 1–100 names), `leap_rule` (Text: `none` \| `every_n`), `leap_every_n` (Int ≥ 2, with `leap_rule = every_n`), `leap_month` (Entity: the month that gains the day).
- `astro.month`: `index` (Int ≥ 1), `name` (Text), `days` (Int ≥ 1), `calendar` (Entity).
- `astro.season`: `name` (Text), `start_month` (Entity), `start_day` (Int ≥ 1), `end_month` (Entity), `end_day` (Int ≥ 1), `calendar` (Entity). Seasons may wrap the year end.

Months and seasons are entities, so each is validated, checkable, individually editable and reorderable.

### 3.7 Derived values are never stored

Distances, masses, positions, brightness, sunlight and tides are recomputed from the entities on demand. Nothing derived enters the Edit Log, so the world hash keeps its current meaning.

### 3.8 Required change to Constraints [3]

`wb-constraints`' kind catalog is closed (a recorded follow-up). This phase adds the extension point:

- `KindCatalog` — a set of registered kinds with `register(&str)`, seeded by `KindCatalog::with_starter_kinds()`.
- `CheckContext` carries `catalog: &KindCatalog`; `evaluate` uses it instead of the `is_constraint_kind` free function, which remains as a thin wrapper over the starter catalog for existing callers.
- `wb-astro::register(&mut EditLog, &mut CheckerRegistry, &mut KindCatalog)` adds its kinds, validators and checkers.

This is additive: existing behaviour and tests are unchanged, as is the Constraints
golden hash.

`propose_seasons` (§4.5) needs one further additive change: `Suggestion` gains
`creates: Vec<NewEntity>`, because a suggestion that adds a season has to create an
entity and v1 suggestions could only write fields on existing ones. `apply_suggestion`
creates them before applying writes, in the same transaction. This does change
`Suggestion`'s postcard encoding, so the Constraints golden hash is updated in the same
commit — a deliberate update, which is what that golden exists to force.

## 4. Computation

### 4.1 Orbits

Each relationship is a two-body problem solved analytically:

- Missing period or distance comes from Kepler's third law against the parent's mass.
- Position at time *t*: mean anomaly → eccentric anomaly by Newton iteration (**fixed 12 iterations**, no convergence-dependent branching, so results are identical on every target) → true anomaly → position in the orbital plane → rotated by inclination.
- The home world's own orbit is driven by `year_days` × day length, so the lore's calendar sets the orbit rather than the reverse.

### 4.2 Sunlight

`insolation(lat, day) -> f64` (W/m², daily mean at the top of the atmosphere): for each star, take its luminosity and its distance from the home world on that day, compute the daily-mean flux for the latitude from the solar elevation integrated over the local day (standard obliquity geometry), and **sum over all stars**. A companion star brightening part of the year falls out of its orbit, not a fudge factor.

`daylight_hours(lat, day) -> f64` from the primary star's hour angle, in local hours, including polar day and night. (A companion contributes light but the "day" is defined by the primary.)

### 4.3 Tides

`tide_range_m(day) -> f64`: equilibrium tide from each moon of the home world and each star, combined as the sum of the individual tidal potentials with their relative phases, so several moons beat against each other and spring and neap tides emerge. Reported as a range in metres.

### 4.4 Positions, phases, brightness

- `position(body, day) -> Vec3` relative to the home world.
- `phase(body, day) -> f64` in 0..1 (illuminated fraction).
- `apparent_brightness(body, day) -> f64` from radius, albedo, distance and phase, as a magnitude-like value so the brightest bodies can be ranked. This is what makes "bright morning star" checkable.

### 4.5 Calendar

- `date_of(day_index) -> Date { year, month_index, day, weekday_index }` and `day_index_of(Date) -> i64`, round-trip exact across the whole supported range (±10 million days), honouring the leap rule.
- `season_of(day) -> Option<&str>` from the named ranges, wrap-around included.
- `propose_seasons(n) -> Vec<Suggestion>`: find the sunlight curve's peaks, troughs and steepest slopes for the home world at mid-latitude, cut the year into *n* ranges at those turning points, and emit Constraints suggestions that create `astro.season` entities with descriptive placeholder names. Proposed seasons always tile the year exactly, with no gaps or overlaps.

## 5. API

```rust
pub fn register(log: &mut EditLog, registry: &mut CheckerRegistry, catalog: &mut KindCatalog);
pub struct SkyModel;
impl SkyModel {
    pub fn build(state: &State) -> Result<SkyModel, AstroError>;
    pub fn insolation(&self, lat: LatLon, day: f64) -> f64;
    pub fn daylight_hours(&self, lat: LatLon, day: f64) -> f64;
    pub fn tide_range_m(&self, day: f64) -> f64;
    pub fn position(&self, body: EntityId, day: f64) -> Option<Vec3>;
    pub fn phase(&self, body: EntityId, day: f64) -> Option<f64>;
    pub fn apparent_brightness(&self, body: EntityId, day: f64) -> Option<f64>;
    pub fn date_of(&self, day_index: i64) -> Date;
    pub fn day_index_of(&self, date: Date) -> Option<i64>;
    pub fn season_of(&self, day: f64) -> Option<&str>;
    pub fn propose_seasons(&self, n: usize) -> Vec<Suggestion>;
    pub fn bodies(&self) -> impl Iterator<Item = &BodyView>;   // derived distances, masses, periods
}
```

`day` is a floating-point day number within the year (0 ≤ day < year_days); `day_index` is an absolute day since the epoch.

## 6. Feasibility checkers

All register with `wb-constraints`; every finding carries the computed numbers and, where a fix exists, one-click suggestions.

| Checker id | Issue code | Badness | Fires when |
|---|---|---|---|
| `astro.companion` | `astro.companion_destabilizes` | 0.9 | A body sits in the unstable annulus between the two Holman–Wiegert criteria — outside the S-type (circumstellar) critical distance, where it would orbit one star safely, and inside the P-type (circumbinary) critical distance, beyond which it would orbit the pair safely. Both bounds are required: the inner one alone reports distant circumbinary bodies as doomed. Names the bodies. Fires only above the stellar mass limit (0.08 M☉) — below it the object is not a star and cannot destabilize anything as one. Suggestions: move beyond the outermost body; demote it to 0.07 M☉, clearly below the stellar limit rather than at it, so the object the checker now exempts is the object the fix actually produces. |
| `astro.companion` | `astro.companion_period_mismatch` | 0.8 | A companion star's **stated** `period_years` differs by more than 25% from the period its distance implies (or, when only the period is stated, the distance that period implies falls inside another body's orbit). Writing "it returns every year" is entering `period_years = 1 local year`, so the clash becomes explicit. The finding states both numbers and the distance each implies. |
| `astro.ordering` | `astro.order_mismatch` | scaled | Stated `order` disagrees with derived distances. Badness = min(1, places_out_of_sequence / 4). Suggestion: renumber. |
| `astro.orbits` | `astro.orbit_crossing` | 0.5 | Two orbits' perihelion/aphelion ranges overlap — unless their periods are within 2% of a simple ratio (≤ 5:4 denominators), which instead emits `astro.resonance_protects` as a **support**. |
| `astro.orbits` | `astro.resonance` | — | **Support**: period ratios within 2% of a simple ratio. Names the pair and the ratio. |
| `astro.moons` | `astro.moon_outside_hill` | 0.9 | A moon's distance exceeds one third of its planet's Hill radius. |
| `astro.moons` | `astro.moon_inside_roche` | 0.9 | A moon orbits inside the Roche limit for its density. |
| `astro.belt` | `astro.belt_overlaps_orbit` | 0.5 | A belt's `inner_au`–`outer_au` span contains another body's orbit. |
| `astro.climate` | `astro.outside_habitable_zone` | 0.6 | Home-world equilibrium temperature (all stars, its albedo) outside 250–320 K. Suggestion: the distance that lands mid-range. |
| `astro.calendar` | `astro.calendar_sum_mismatch` | 0.9 | Months don't sum to `year_days` (within the leap rule). Suggestion: adjust the longest month. |
| `astro.calendar` | `astro.leap_drift` | 0.6 | The leap rule leaves more than 0.01 days/year of drift. |
| `astro.calendar` | `astro.month_index_duplicated` | 0.9 | Two or more live months of the same calendar share an `index`, which silently breaks `date_of`/`day_index_of`'s round-trip for every date that lands on the duplicated month. Names the index and the months. No suggestion: which index should move is the user's call. |
| `astro.body` | `astro.period_distance_conflict` | 0.7 | Both `period_years` and `distance_au` given and inconsistent by more than 5%. Suggestion: keep one. |
| `astro.spin` | `astro.tidal_locking` | — | **Consequence**: estimated locking time shorter than a typical civilisation's history. |

Every finding can be kept anyway with a reason, which marks it intentional. The realism dial grades these like any other constraint.

## 7. Errors

`AstroError` (returned only by `SkyModel::build`): `NoPrimaryStar`, `MultiplePrimaryStars`, `MissingParent(EntityId)`, `ParentCycle(EntityId)`, `NoCalendar`, `NoMonths`. Everything else — contradictory, unstable or absurd — is a verdict, never an error. Checkers never panic and never error.

## 8. Determinism

All Phase-1 rules hold: `libm` only (no platform transcendentals), no FMA, sorted collections, no randomness. The orbit solver uses a fixed iteration count rather than a convergence test. `SkyModel`'s outputs must be bit-identical on aarch64, wasm32-wasip1 and x86-64, checked by a golden hash over a fixed system's computed values.

## 9. Testing

- **Earth validation:** real Sun/Earth/Moon reproduce published values — annual mean insolation at the equator and pole within 2%, the solstice/equinox daylight pattern, and spring tides roughly twice neap tides.
- **Solar-system validation:** real planetary distances reproduce real periods within 1%; Neptune and Pluto produce `astro.resonance_protects`, not `astro.orbit_crossing`.
- **Aethoria acceptance:** the thirteen-body system, three moons, two suns, 30-hour day, 390-day year, 13 months. Asserts: the three moons beat into a maximum spring tide on a repeatable day; the calendar round-trips; Sol Secundus produces both `astro.companion_destabilizes` and `astro.companion_period_mismatch` with real numbers; the resonance checker names at least one pair.
- **Property tests:** `date_of`/`day_index_of` round-trip over 100 000 consecutive days; proposed seasons tile the year exactly; insolation is hemisphere-symmetric for a circular, untilted orbit; `tide_range_m` is finite and non-negative everywhere.
- **Cross-target golden:** a fixed system's computed values hash identically on all three targets (via `scripts/ci-local.sh`).
- **Performance guard:** a 13-body system with 20 moons computes a full year of daily
  insolation and tides in ~2.3-2.4 ms native (wasm not measured), against a budget of
  50 ms; evaluating every checker over the same system takes ~2.7-2.9 ms native, against
  a 50 ms budget. Both guards time in microseconds and share a 150 ms wasm budget.
  Release-only, wired into `scripts/check.sh`.

## 10. Out of scope

Drawing the sky (Renderer [11]); the eclipse and conjunction event catalogue on the timeline (designed for, built later); N-body perturbations; editing raw orbital elements; magical/fantasy overrides of orbital mechanics (Custom Fields [14]).
