# Constraints & Feasibility — follow-ups

Deliberately deferred out of the v1 Constraints & Feasibility branch
(`docs/superpowers/specs/2026-09-22-constraints-design.md`). Nothing here is a known
defect in what shipped; each is a boundary that the next subsystem will push on.

## (a) The kind catalog is a closed `const`

`kinds::CONSTRAINT_KINDS` is a `&[&str]` baked into `wb-constraints`, and
`is_constraint_kind` is the only gate `evaluate` applies. An entity whose kind is not
listed is silently ignored — no verdict, no error, no warning. Correct for
`import.base_map` and friends; wrong the moment another subsystem wants to contribute a
kind.

The intended extension is a catalog carried in `CheckContext`, registered at evaluation
time instead of compiled in. Decide the shape before **Pipeline [4]**; **Custom Fields
[14]** and **Codex [16]** both need it.

## (b) `CheckContext` has no per-evaluation memo or index

Every "entities near X" checker rescans `ctx.state.live()` from scratch:
`RegionRulesCheck` does it once per rule, `RiverMouthCheck` once per river. The
bounding-box prefilter removed the O(ring) winding test from the inner loop, but the
scan itself — and the `centroid` of each candidate, which is three trig calls per vertex
— is still recomputed for every constraint that asks.

On the 5 000-constraint perf fixture this is the dominant cost (≈180 ms native, from
≈1 180 ms before the prefilter). The fix is a per-evaluation slot on `CheckContext`: a
memo of candidate centroids, or a coarse spatial index keyed by kind. It has to be built
once per `evaluate` and stay deterministic. **Decide before Pipeline [4]**, which adds
simulation-backed checkers that will scan far more.

## (c) `templates::fill` substitutes params sequentially

`fill` walks the param map and calls `str::replace` once per key, so a param *value*
containing another key's placeholder — `{ratio}`, `{unit}` — is itself substituted by a
later pass. Today every param value is machine-generated (a number, a unit, a kind), so
the bug is unreachable.

**Codex [16]** puts free text from imported lore into params, at which point it is
reachable and produces silently wrong explanation strings. Replace `fill` with a
single-pass scan over the template before that lands.

## (d) Degenerate geometry grades as Plausible

A ring with fewer than 3 points has no inside: `contains` returns false and `bbox`
returns `None`, both by contract. A 2-point or empty `area` therefore produces no
finding at all from `RegionRulesCheck`, `RiverMouthCheck` or `LoreAreaCheck`, and the
constraint lands on score 1.0 → Plausible. The checker rule "malformed input yields no
finding" is being applied to something that is not malformed input but a malformed
*world*: the user drew a degenerate shape and the report says it is fine.

Consider a `geometry.degenerate` **Consequence** (badness 0, so it does not move the
score) in v1.5, surfaced in the report as "this shape cannot be evaluated".

## (e) Spec §7 omits `lore.area_mismatch`'s `related`

`LoreAreaCheck` attaches `.with_related(subject)` to the `lore.area_mismatch` issue —
the drawn feature the claim is about — so the Editor can pin the finding to the right
shape on the map. The spec's §7 table lists the issue's params and badness but not the
`related` entity, unlike the `rule.forbidden_feature` row, which does record it. Add the
`related` = subject note to that row.

## (f) Deferred minors

- `ConstraintError` does not implement `std::error::Error::source()`, so an
  `Edit(EditError)` does not chain to its cause for callers using `anyhow`-style
  reporting.
- `coast.rs`: the `pts.is_empty()` disjunct in `if ocean == 0 || pts.is_empty()` is
  dead — `ocean > 0` already implies at least one sample. Harmless, but it reads as a
  guard that is doing something.
- `region_rules.rs`: the `other.id == c.id` self-skip is dead, because a `rule.region`
  is never in its own `forbidden()` list. Same story: harmless, misleading.
- `kept()` in `actions.rs` returns `(codes, display_name)`; the name says only half of
  what it does.
- `river_mouth.rs` has no test for the `UnknownTerrain` path (terrain answers `None`, so
  neither issue can fire). The behaviour is right; it is simply unpinned.
