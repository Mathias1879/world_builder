# World Model & Spherical Grid — follow-ups

Deferred from task reviews and the final whole-branch review of `feat/world-model`
(2026-09-22). None affects this subsystem's guarantees; each names the subsystem that
should pick it up.

## Verification record
- Golden world hash `58ecf1d9ef5f2c9e307ffac5d7b9f6457fb44c9b03ad2e53c7102cb8f9185e56` matched on:
  native aarch64-apple-darwin (debug + release), wasm32-wasip1 (wasmtime), and
  x86_64-unknown-linux-musl (Apple Container, amd64 Alpine). Full suite green on all three.
- GitHub Actions has not run yet (account billing lock). Local x86-64 recipe:
  `rustup target add x86_64-unknown-linux-musl`,
  `CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld cargo test --workspace --no-run --target x86_64-unknown-linux-musl`,
  then run the binaries in `container run --rm --arch amd64 -v <bins>:/t alpine:3.20`.

## Decisions made during execution
- `TileId::cell` asserts local bounds in release builds too.
- Cross-face adjacency is an exact integer table; at the 24 cube-corner cells the diagonal
  into the missing fourth cell resolves to the neighbour across the i-edge (deduped by `neighbors8`).
- `RegionBox` rejects only west=+180°/east=−180° as zero width; −180..180 is the full circle.
- Extent tile selection is conservative (may over-include tiles within ~1e-9 rad of the box).
- NaN is canonicalized (f32 `0x7FC0_0000`, f64 `0x7FF8_0000_0000_0000`) before encoding/hashing.

## Follow-ups by owner
**Edit Log [2]**
- Canonicalize lat/lon at the log boundary (`LatLon::from_degrees` does not wrap lon into (−π, π]).
- Always construct `TileId`/`CellId` via `new()`; fields are public and bypass validation.

**Pipeline stage contract [4]**
- Regional extents at level ≥ 1: border tiles need neighbours outside the selection, so
  `gather_with_apron` returns `MissingTile` — define a fallback (e.g. coarser level or context tiles).
- `downsample` takes the LOD policy as an argument; add a `World`-level wrapper that uses the registered policy.
- `refine_tile` evaluates `detail` and sibling areas 4× per parent; compute once per parent (requires pure `detail`).

**Hashing / determinism**
- `world_hash` should include the layer count (theoretical framing ambiguity) — do it in the next commit that changes GOLDEN anyway.
- Bump `ENGINE_VERSION` whenever GOLDEN changes (spec §2.3 item 4); note it in `tests/golden.rs`.
- Widen golden coverage: `downsample`, `locate`/`offset`/apron output, `Extent::tiles`, geometry functions.
- `Tile::decode` does not check reserved header byte 7 is zero.
- Guard `CellId::locate`, `TileId::all`, `cells_per_edge` with `level <= MAX_LEVEL` (shift UB-like behaviour ≥ 32).
- `TileId::all(high level)` allocates ~1.6 GB at level 12 — document or iterate lazily.

**CI**
- GitHub Actions is manual-only until alpha/beta (no paid services); `scripts/ci-local.sh` is the merge gate.
- If Actions is re-enabled: replace `rustup show` with explicit `rustup toolchain install` (rustup ≥ 1.28 no longer auto-installs via `show`).

**Tests / polish (low priority)**
- Mode third tie-break (equal count + area) untested; `pick_max` self-compares `v[0]`.
- `FeatureSet::insert` overwrite return untested; `polyline_length_m` short-slice 0.0 is implicit.
- `LayerStore::get/get_mut/iter` not directly tested; `register_layer` has 4 near-identical arms.
- `area_unit` inclusion-exclusion loses ~1e-4 relative precision at MAX_LEVEL (weights stay self-consistent).
- `to_hex` allocates per byte; doc gaps on `Face::index/from_index`, `ApronError`, `sphere_to_face` zero vector.
