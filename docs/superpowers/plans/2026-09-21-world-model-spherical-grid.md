# World Model & Spherical Grid Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build subsystem [1] — the deterministic spherical grid (`wb-grid`) and world data model (`wb-world`) that every other World_Builder subsystem stands on.

**Architecture:** An equi-angular cube-sphere (6 faces), each face a quadtree of 256×256-cell tiles. `wb-grid` is pure geometry: coordinates, cell/tile indexing, cross-face neighbours, exact cell areas. `wb-world` holds typed per-cell layers in tiles, a layer registry, level-of-detail operators that preserve the area-weighted mean ("coarse is truth"), seam-safe tile aprons, project extents, spherical vector geometry, and content-addressed hashing. Determinism across WASM, x86-64, and ARM64 is enforced by lint rules plus a golden-hash test run on all three targets in CI.

**Tech Stack:** Rust 1.90.0 (edition 2024), `libm` (pure-Rust math), `blake3` (hashing, `pure` feature), `proptest` (property tests), `wasmtime` (runs `wasm32-wasip1` tests), GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-09-21-world-builder-architecture-design.md` — §2.3 (invariants) and §3 (World Model & Spherical Grid).

## Global Constraints

- Toolchain pinned: Rust `1.90.0`, edition `2024`, targets native + `wasm32-wasip1`.
- Determinism: `(inputs, seed, engine version)` → bit-identical results on WASM, native x86-64, native ARM64.
- No platform math: every transcendental function (`sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `exp`, `ln`, `powf`, `powi`, `hypot`, …) goes through `libm::`. `sqrt` and basic arithmetic are IEEE-exact and allowed.
- No FMA: `mul_add` is forbidden.
- No `HashMap`/`HashSet` in engine crates — use `BTreeMap`/`BTreeSet` so iteration order is deterministic.
- No `unsafe` code.
- No order-dependent parallel reductions (this plan is single-threaded).
- Coordinates: unit sphere, `z` = north pole, `x` = latitude 0 / longitude 0, `y` = longitude 90°E. Angles in radians internally.
- Tile size 256×256 cells; tile levels `0..=MAX_LEVEL` (12); cell storage row-major `j * 256 + i`.
- All commands run from the repository root `~/dev/world_builder`.
- Run `cargo fmt --all` before every `scripts/check.sh` gate; the gate fails on unformatted code.
- Commits end with the trailer `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.

## Out of scope (owned by later subsystems)

Registering the concrete physical layers (`elevation`, `temperature`, …) — each pipeline stage registers its own. "Water runs downhill at every level" — Hydrology spec. "Detail follows the landscape" — Terrain spec supplies the `detail` function that `refine_tile` consumes. Synthesizing level-0 context around regional projects — Pipeline spec. Edit log — subsystem [2].

## File Structure

```
Cargo.toml                         workspace, shared deps, lints, profiles
rust-toolchain.toml                pinned toolchain + wasm target
clippy.toml                        determinism lint lists
.cargo/config.toml                 wasmtime as wasm32-wasip1 test runner
scripts/check.sh                   fmt + clippy + native tests + wasm tests
.github/workflows/ci.yml           same checks on x86-64, ARM64 (Linux + macOS), WASM
crates/wb-grid/
  Cargo.toml
  src/lib.rs                       re-exports
  src/vec3.rs                      Vec3 math
  src/latlon.rs                    LatLon <-> Vec3
  src/face.rs                      Face enum, equi-angular projection
  src/cell.rs                      TileId, CellId, indexing, locate, parent/children
  src/neighbors.rs                 cross-face neighbour lookup
  src/area.rs                      exact spherical cell areas
  tests/determinism_canary.rs  tests/latlon.rs  tests/face.rs
  tests/cell.rs  tests/neighbors.rs  tests/area.rs
crates/wb-world/
  Cargo.toml
  src/lib.rs                       re-exports
  src/tile.rs                      Dtype, CellValue, Tile<T>, encode/decode
  src/extent.rs                    Extent, RegionBox, tiles-in-extent
  src/layer.rs                     LayerId, LayerDesc, LodPolicy, LayerRegistry
  src/world.rs                     Planet, LayerStore, World, ENGINE_VERSION
  src/lod.rs                       downsample / refine (coarse is truth)
  src/apron.rs                     seam-safe tile aprons
  src/geometry.rs                  great-circle distance, length, polygon area
  src/feature.rs                   vector features
  src/hash.rs                      SourceHash, tile cache keys, world hash
  tests/tile.rs  tests/extent.rs  tests/world.rs  tests/lod.rs
  tests/apron.rs  tests/geometry.rs  tests/golden.rs
```

---

### Task 1: Workspace scaffold and determinism guardrails

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `clippy.toml`, `.cargo/config.toml`, `scripts/check.sh`
- Create: `crates/wb-grid/Cargo.toml`, `crates/wb-grid/src/lib.rs`
- Test: `crates/wb-grid/tests/determinism_canary.rs`
- Modify: `.gitignore`, `README.md`

**Interfaces:**
- Consumes: nothing.
- Produces: a workspace whose `members = ["crates/*"]` picks up any new crate automatically; workspace dependencies `libm`, `blake3`, `proptest`, `wb-grid`; `scripts/check.sh` used by every later task.

- [ ] **Step 1: Install tools**

```bash
brew install wasmtime
wasmtime --version
```
Expected: a version line such as `wasmtime 3x.x.x`.

- [ ] **Step 2: Write the workspace files**

`Cargo.toml`:
```toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.90"
license = "LicenseRef-Proprietary"
publish = false

[workspace.dependencies]
libm = "0.2"
blake3 = { version = "1", features = ["pure"] }
proptest = { version = "1", default-features = false, features = ["std"] }
wb-grid = { path = "crates/wb-grid" }

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
disallowed_methods = "deny"
disallowed_types = "deny"

[profile.dev.package."*"]
opt-level = 2

[profile.test]
opt-level = 1
```

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "1.90.0"
components = ["rustfmt", "clippy"]
targets = ["wasm32-wasip1"]
```

`clippy.toml`:
```toml
disallowed-methods = [
  { path = "f64::sin", reason = "use libm::sin (determinism)" },
  { path = "f64::cos", reason = "use libm::cos (determinism)" },
  { path = "f64::tan", reason = "use libm::tan (determinism)" },
  { path = "f64::sin_cos", reason = "use libm::sin and libm::cos (determinism)" },
  { path = "f64::asin", reason = "use libm::asin (determinism)" },
  { path = "f64::acos", reason = "use libm::acos (determinism)" },
  { path = "f64::atan", reason = "use libm::atan (determinism)" },
  { path = "f64::atan2", reason = "use libm::atan2 (determinism)" },
  { path = "f64::sinh", reason = "use libm::sinh (determinism)" },
  { path = "f64::cosh", reason = "use libm::cosh (determinism)" },
  { path = "f64::tanh", reason = "use libm::tanh (determinism)" },
  { path = "f64::exp", reason = "use libm::exp (determinism)" },
  { path = "f64::exp2", reason = "use libm::exp2 (determinism)" },
  { path = "f64::exp_m1", reason = "use libm::expm1 (determinism)" },
  { path = "f64::ln", reason = "use libm::log (determinism)" },
  { path = "f64::ln_1p", reason = "use libm::log1p (determinism)" },
  { path = "f64::log", reason = "use libm (determinism)" },
  { path = "f64::log2", reason = "use libm::log2 (determinism)" },
  { path = "f64::log10", reason = "use libm::log10 (determinism)" },
  { path = "f64::powf", reason = "use libm::pow (determinism)" },
  { path = "f64::powi", reason = "use repeated multiplication or libm::pow (determinism)" },
  { path = "f64::cbrt", reason = "use libm::cbrt (determinism)" },
  { path = "f64::hypot", reason = "use libm::hypot (determinism)" },
  { path = "f64::mul_add", reason = "FMA changes rounding across targets" },
  { path = "f32::sin", reason = "use libm::sinf (determinism)" },
  { path = "f32::cos", reason = "use libm::cosf (determinism)" },
  { path = "f32::tan", reason = "use libm::tanf (determinism)" },
  { path = "f32::sin_cos", reason = "use libm::sinf and libm::cosf (determinism)" },
  { path = "f32::asin", reason = "use libm::asinf (determinism)" },
  { path = "f32::acos", reason = "use libm::acosf (determinism)" },
  { path = "f32::atan", reason = "use libm::atanf (determinism)" },
  { path = "f32::atan2", reason = "use libm::atan2f (determinism)" },
  { path = "f32::exp", reason = "use libm::expf (determinism)" },
  { path = "f32::ln", reason = "use libm::logf (determinism)" },
  { path = "f32::powf", reason = "use libm::powf (determinism)" },
  { path = "f32::powi", reason = "use repeated multiplication or libm::powf (determinism)" },
  { path = "f32::hypot", reason = "use libm::hypotf (determinism)" },
  { path = "f32::mul_add", reason = "FMA changes rounding across targets" },
]
disallowed-types = [
  { path = "std::collections::HashMap", reason = "nondeterministic iteration order; use BTreeMap" },
  { path = "std::collections::HashSet", reason = "nondeterministic iteration order; use BTreeSet" },
]
```

`.cargo/config.toml`:
```toml
[target.wasm32-wasip1]
runner = "wasmtime"
```

`scripts/check.sh`:
```bash
#!/usr/bin/env bash
# Full local gate: formatting, lints, native tests, WASM tests.
set -euo pipefail
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --target wasm32-wasip1
```
Then: `chmod +x scripts/check.sh`

`crates/wb-grid/Cargo.toml`:
```toml
[package]
name = "wb-grid"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
libm.workspace = true

[dev-dependencies]
proptest.workspace = true

[lints]
workspace = true
```

`crates/wb-grid/src/lib.rs`:
```rust
//! Spherical grid: an equi-angular cube-sphere split into quadtree tiles.
```

Append to `.gitignore`:
```
/target
```

Append to `README.md`:
```markdown
## Development

Requires `rustup` and `wasmtime` (`brew install wasmtime`). The toolchain is pinned in `rust-toolchain.toml`.

    scripts/check.sh    # fmt, clippy, native tests, wasm32-wasip1 tests
```

- [ ] **Step 3: Write the determinism canary test**

`crates/wb-grid/tests/determinism_canary.rs`:
```rust
//! If these fail on any target, libm is not bit-exact there and the
//! determinism invariant is broken.

#[test]
fn libm_sin_is_correctly_rounded() {
    assert_eq!(
        libm::sin(1.0).to_bits(),
        0.841_470_984_807_896_5_f64.to_bits()
    );
}

#[test]
fn libm_atan_of_one_is_quarter_pi() {
    assert_eq!(
        libm::atan(1.0).to_bits(),
        core::f64::consts::FRAC_PI_4.to_bits()
    );
}
```

- [ ] **Step 4: Run the full gate**

Run: `scripts/check.sh`
Expected: formatting and clippy clean; `2 passed` natively and `2 passed` under wasm32-wasip1. (The first run downloads the pinned toolchain and the wasm target.)

- [ ] **Step 5: Prove the lint guardrail works, then revert**

Temporarily append to `crates/wb-grid/src/lib.rs`:
```rust
pub fn bad(x: f64) -> f64 {
    x.sin()
}
```
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: FAIL with `use of a disallowed method `f64::sin`` and the reason `use libm::sin (determinism)`.
Remove the `bad` function again and re-run clippy — expected clean.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml rust-toolchain.toml clippy.toml .cargo scripts crates .gitignore README.md Cargo.lock
git commit -m "build: Rust workspace with determinism guardrails and wasm test runner" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: Vec3 and LatLon

**Files:**
- Create: `crates/wb-grid/src/vec3.rs`, `crates/wb-grid/src/latlon.rs`
- Modify: `crates/wb-grid/src/lib.rs`
- Test: `crates/wb-grid/tests/latlon.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub struct Vec3 { pub x: f64, pub y: f64, pub z: f64 }` — `const fn new`, `plus(self, Vec3) -> Vec3`, `scaled(self, f64) -> Vec3`, `dot(self, Vec3) -> f64`, `cross(self, Vec3) -> Vec3`, `length(self) -> f64`, `normalize(self) -> Vec3`.
  - `pub struct LatLon { pub lat: f64, pub lon: f64 }` (radians; `lon` in `(-π, π]`) — `from_degrees(f64, f64) -> LatLon`, `to_vec3(self) -> Vec3`, `from_vec3(Vec3) -> LatLon`.

- [ ] **Step 1: Write the failing tests**

`crates/wb-grid/tests/latlon.rs`:
```rust
use proptest::prelude::*;
use wb_grid::{LatLon, Vec3};

fn close(a: Vec3, b: Vec3, eps: f64) -> bool {
    (a.x - b.x).abs() < eps && (a.y - b.y).abs() < eps && (a.z - b.z).abs() < eps
}

#[test]
fn cardinal_points() {
    assert!(close(LatLon::from_degrees(0.0, 0.0).to_vec3(), Vec3::new(1.0, 0.0, 0.0), 1e-15));
    assert!(close(LatLon::from_degrees(0.0, 90.0).to_vec3(), Vec3::new(0.0, 1.0, 0.0), 1e-15));
    assert!(close(LatLon::from_degrees(90.0, 0.0).to_vec3(), Vec3::new(0.0, 0.0, 1.0), 1e-15));
}

#[test]
fn vec3_algebra() {
    let a = Vec3::new(1.0, 0.0, 0.0);
    let b = Vec3::new(0.0, 1.0, 0.0);
    assert_eq!(a.cross(b), Vec3::new(0.0, 0.0, 1.0));
    assert_eq!(a.dot(b), 0.0);
    assert_eq!(Vec3::new(3.0, 4.0, 0.0).length(), 5.0);
    assert_eq!(a.plus(b).scaled(2.0), Vec3::new(2.0, 2.0, 0.0));
    assert!((Vec3::new(2.0, 0.0, 0.0).normalize().length() - 1.0).abs() < 1e-15);
}

fn cfg() -> ProptestConfig {
    ProptestConfig { cases: 256, failure_persistence: None, ..ProptestConfig::default() }
}

proptest! {
    #![proptest_config(cfg())]
    #[test]
    fn roundtrip(lat in -89.9f64..89.9, lon in -179.9f64..179.9) {
        let p = LatLon::from_degrees(lat, lon);
        let q = LatLon::from_vec3(p.to_vec3());
        prop_assert!((p.lat - q.lat).abs() < 1e-12);
        prop_assert!((p.lon - q.lon).abs() < 1e-12);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p wb-grid --test latlon`
Expected: FAIL — `unresolved imports wb_grid::LatLon, wb_grid::Vec3`.

- [ ] **Step 3: Implement**

`crates/wb-grid/src/vec3.rs`:
```rust
/// A 3-vector of f64. Only IEEE-exact operations (+, -, *, /, sqrt) are used.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn plus(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }

    pub fn scaled(self, k: f64) -> Vec3 {
        Vec3::new(self.x * k, self.y * k, self.z * k)
    }

    pub fn dot(self, o: Vec3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    pub fn normalize(self) -> Vec3 {
        let l = self.length();
        Vec3::new(self.x / l, self.y / l, self.z / l)
    }
}
```

`crates/wb-grid/src/latlon.rs`:
```rust
use crate::vec3::Vec3;

/// Geographic coordinate in radians. `lat` in [-π/2, π/2], `lon` in (-π, π].
/// Convention: z = north pole, x = (0°, 0°), y = (0°, 90°E).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

impl LatLon {
    pub fn from_degrees(lat_deg: f64, lon_deg: f64) -> Self {
        Self { lat: lat_deg.to_radians(), lon: lon_deg.to_radians() }
    }

    pub fn to_vec3(self) -> Vec3 {
        let cl = libm::cos(self.lat);
        Vec3::new(cl * libm::cos(self.lon), cl * libm::sin(self.lon), libm::sin(self.lat))
    }

    pub fn from_vec3(v: Vec3) -> Self {
        let horizontal = (v.x * v.x + v.y * v.y).sqrt();
        Self { lat: libm::atan2(v.z, horizontal), lon: libm::atan2(v.y, v.x) }
    }
}
```

`crates/wb-grid/src/lib.rs`:
```rust
//! Spherical grid: an equi-angular cube-sphere split into quadtree tiles.

mod latlon;
mod vec3;

pub use latlon::LatLon;
pub use vec3::Vec3;
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p wb-grid --test latlon`
Expected: PASS (3 tests).

- [ ] **Step 5: Gate and commit**

Run: `scripts/check.sh` — expected all green.
```bash
git add crates/wb-grid
git commit -m "feat(grid): Vec3 and LatLon conversions" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: Cube faces and equi-angular projection

**Files:**
- Create: `crates/wb-grid/src/face.rs`
- Modify: `crates/wb-grid/src/lib.rs`
- Test: `crates/wb-grid/tests/face.rs`

**Interfaces:**
- Consumes: `Vec3` (Task 2).
- Produces:
  - `#[repr(u8)] pub enum Face { PosX = 0, NegX = 1, PosY = 2, NegY = 3, PosZ = 4, NegZ = 5 }` deriving `Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash`; `Face::ALL: [Face; 6]`; `index(self) -> u8`; `from_index(u8) -> Option<Face>`; `basis(self) -> (Vec3, Vec3, Vec3)` returning `(normal, u_axis, v_axis)`.
  - `pub fn face_to_sphere(face: Face, s: f64, t: f64) -> Vec3` — equi-angular `s, t ∈ [-1, 1]` (values slightly beyond ±1 are allowed and land on the adjacent face) → unit vector.
  - `pub fn sphere_to_face(p: Vec3) -> (Face, f64, f64)` — any non-zero vector → face and `s, t` clamped to `[-1, 1]`. Ties in the major axis resolve in the order x, then y, then z.

- [ ] **Step 1: Write the failing tests**

`crates/wb-grid/tests/face.rs`:
```rust
use proptest::prelude::*;
use wb_grid::{face_to_sphere, sphere_to_face, Face};

#[test]
fn face_centres_are_normals() {
    for face in Face::ALL {
        let (n, _, _) = face.basis();
        let p = face_to_sphere(face, 0.0, 0.0);
        assert!((p.dot(n) - 1.0).abs() < 1e-15, "{face:?}");
    }
}

#[test]
fn bases_are_orthonormal() {
    for face in Face::ALL {
        let (n, u, v) = face.basis();
        assert_eq!(n.dot(u), 0.0);
        assert_eq!(n.dot(v), 0.0);
        assert_eq!(u.dot(v), 0.0);
    }
}

#[test]
fn index_roundtrip() {
    for face in Face::ALL {
        assert_eq!(Face::from_index(face.index()), Some(face));
    }
    assert_eq!(Face::from_index(6), None);
}

#[test]
fn stepping_past_an_edge_changes_face() {
    // +u on PosX points toward +Y.
    let (face, _, _) = sphere_to_face(face_to_sphere(Face::PosX, 1.01, 0.0));
    assert_eq!(face, Face::PosY);
}

fn face_strategy() -> impl Strategy<Value = Face> {
    (0u8..6).prop_map(|i| Face::from_index(i).unwrap())
}

fn cfg() -> ProptestConfig {
    ProptestConfig { cases: 512, failure_persistence: None, ..ProptestConfig::default() }
}

proptest! {
    #![proptest_config(cfg())]
    #[test]
    fn projection_roundtrip(face in face_strategy(), s in -0.999f64..0.999, t in -0.999f64..0.999) {
        let p = face_to_sphere(face, s, t);
        prop_assert!((p.length() - 1.0).abs() < 1e-14);
        let (f2, s2, t2) = sphere_to_face(p);
        prop_assert_eq!(f2, face);
        prop_assert!((s - s2).abs() < 1e-12);
        prop_assert!((t - t2).abs() < 1e-12);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p wb-grid --test face`
Expected: FAIL — unresolved imports `face_to_sphere`, `sphere_to_face`, `Face`.

- [ ] **Step 3: Implement**

`crates/wb-grid/src/face.rs`:
```rust
use crate::vec3::Vec3;
use core::f64::consts::FRAC_PI_4;

const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);
const NX: Vec3 = Vec3::new(-1.0, 0.0, 0.0);
const NY: Vec3 = Vec3::new(0.0, -1.0, 0.0);
const NZ: Vec3 = Vec3::new(0.0, 0.0, -1.0);

/// One of the six cube faces. Declaration order defines `Ord` and must not change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Face {
    PosX = 0,
    NegX = 1,
    PosY = 2,
    NegY = 3,
    PosZ = 4,
    NegZ = 5,
}

impl Face {
    pub const ALL: [Face; 6] = [Face::PosX, Face::NegX, Face::PosY, Face::NegY, Face::PosZ, Face::NegZ];

    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn from_index(i: u8) -> Option<Face> {
        Face::ALL.get(i as usize).copied()
    }

    /// `(normal, u_axis, v_axis)`; cube point = normal + a·u + b·v.
    pub fn basis(self) -> (Vec3, Vec3, Vec3) {
        match self {
            Face::PosX => (X, Y, Z),
            Face::NegX => (NX, NY, Z),
            Face::PosY => (Y, NX, Z),
            Face::NegY => (NY, X, Z),
            Face::PosZ => (Z, Y, NX),
            Face::NegZ => (NZ, Y, X),
        }
    }
}

/// Equi-angular face coordinates → unit vector. `s`/`t` beyond ±1 (up to < 2)
/// land on the neighbouring face, which neighbour lookup relies on.
pub fn face_to_sphere(face: Face, s: f64, t: f64) -> Vec3 {
    let (n, u, v) = face.basis();
    let a = libm::tan(FRAC_PI_4 * s);
    let b = libm::tan(FRAC_PI_4 * t);
    n.plus(u.scaled(a)).plus(v.scaled(b)).normalize()
}

/// Any non-zero vector → (face, s, t) with s, t clamped to [-1, 1].
pub fn sphere_to_face(p: Vec3) -> (Face, f64, f64) {
    let (ax, ay, az) = (p.x.abs(), p.y.abs(), p.z.abs());
    let face = if ax >= ay && ax >= az {
        if p.x >= 0.0 { Face::PosX } else { Face::NegX }
    } else if ay >= az {
        if p.y >= 0.0 { Face::PosY } else { Face::NegY }
    } else if p.z >= 0.0 {
        Face::PosZ
    } else {
        Face::NegZ
    };
    let (n, u, v) = face.basis();
    let d = p.dot(n);
    let s = libm::atan(p.dot(u) / d) / FRAC_PI_4;
    let t = libm::atan(p.dot(v) / d) / FRAC_PI_4;
    (face, s.clamp(-1.0, 1.0), t.clamp(-1.0, 1.0))
}
```

Add to `crates/wb-grid/src/lib.rs`:
```rust
mod face;
pub use face::{face_to_sphere, sphere_to_face, Face};
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p wb-grid --test face`
Expected: PASS (5 tests).

- [ ] **Step 5: Gate and commit**

Run: `scripts/check.sh` — expected all green (run `cargo fmt --all` first if the format check complains).
```bash
git add crates/wb-grid
git commit -m "feat(grid): cube faces with equi-angular projection" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: Tile and cell indexing

**Files:**
- Create: `crates/wb-grid/src/cell.rs`
- Modify: `crates/wb-grid/src/lib.rs`
- Test: `crates/wb-grid/tests/cell.rs`

**Interfaces:**
- Consumes: `Face`, `face_to_sphere`, `sphere_to_face` (Task 3); `Vec3` (Task 2).
- Produces:
  - `pub const TILE_SIZE: u32 = 256;` `pub const MAX_LEVEL: u8 = 12;`
  - `pub const fn tiles_per_edge(level: u8) -> u32` (= `2^level`), `pub const fn cells_per_edge(level: u8) -> u32` (= `256 · 2^level`).
  - `pub struct TileId { pub face: Face, pub level: u8, pub x: u32, pub y: u32 }` deriving `Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash`; `new(face, level, x, y) -> Option<TileId>`; `cell(self, i: u32, j: u32) -> CellId`; `parent(self) -> Option<TileId>`; `children(self) -> Option<[TileId; 4]>`; `all(level: u8) -> Vec<TileId>` (sorted in `Ord` order).
  - `pub struct CellId { pub face: Face, pub level: u8, pub i: u32, pub j: u32 }` (global per-face indices; same derives); `new(face, level, i, j) -> Option<CellId>`; `center_st(self) -> (f64, f64)`; `center(self) -> Vec3`; `locate(p: Vec3, level: u8) -> CellId`; `tile(self) -> (TileId, u32, u32)`; `parent(self) -> Option<CellId>` (None at level 0); `children(self) -> Option<[CellId; 4]>` ordered `(2i,2j), (2i+1,2j), (2i,2j+1), (2i+1,2j+1)`.

- [ ] **Step 1: Write the failing tests**

`crates/wb-grid/tests/cell.rs`:
```rust
use proptest::prelude::*;
use wb_grid::{cells_per_edge, tiles_per_edge, CellId, Face, TileId, MAX_LEVEL, TILE_SIZE};

#[test]
fn sizes() {
    assert_eq!(tiles_per_edge(0), 1);
    assert_eq!(tiles_per_edge(3), 8);
    assert_eq!(cells_per_edge(0), 256);
    assert_eq!(cells_per_edge(MAX_LEVEL), 256 * 4096);
}

#[test]
fn bounds_are_checked() {
    assert!(TileId::new(Face::PosX, 1, 1, 1).is_some());
    assert!(TileId::new(Face::PosX, 1, 2, 0).is_none());
    assert!(TileId::new(Face::PosX, MAX_LEVEL + 1, 0, 0).is_none());
    assert!(CellId::new(Face::NegZ, 0, 255, 255).is_some());
    assert!(CellId::new(Face::NegZ, 0, 256, 0).is_none());
}

#[test]
fn tile_cell_mapping() {
    let t = TileId::new(Face::PosY, 2, 3, 1).unwrap();
    let c = t.cell(10, 20);
    assert_eq!((c.i, c.j), (3 * TILE_SIZE + 10, TILE_SIZE + 20));
    assert_eq!(c.tile(), (t, 10, 20));
}

#[test]
fn tile_hierarchy() {
    let t = TileId::new(Face::NegY, 3, 5, 6).unwrap();
    let kids = t.children().unwrap();
    for k in kids {
        assert_eq!(k.parent(), Some(t));
    }
    assert_eq!(TileId::new(Face::NegY, 0, 0, 0).unwrap().parent(), None);
    assert_eq!(TileId::new(Face::NegY, MAX_LEVEL, 0, 0).unwrap().children(), None);
}

#[test]
fn all_tiles_sorted_and_counted() {
    let all = TileId::all(1);
    assert_eq!(all.len(), 24);
    let mut sorted = all.clone();
    sorted.sort();
    assert_eq!(all, sorted);
}

fn face_strategy() -> impl Strategy<Value = Face> {
    (0u8..6).prop_map(|i| Face::from_index(i).unwrap())
}

fn cell_strategy() -> impl Strategy<Value = CellId> {
    (face_strategy(), 0u8..=8).prop_flat_map(|(face, level)| {
        let m = cells_per_edge(level);
        (0..m, 0..m).prop_map(move |(i, j)| CellId::new(face, level, i, j).unwrap())
    })
}

fn cfg() -> ProptestConfig {
    ProptestConfig { cases: 512, failure_persistence: None, ..ProptestConfig::default() }
}

proptest! {
    #![proptest_config(cfg())]
    #[test]
    fn locate_inverts_center(c in cell_strategy()) {
        prop_assert_eq!(CellId::locate(c.center(), c.level), c);
    }

    #[test]
    fn cell_is_child_of_its_parent(c in cell_strategy()) {
        if let Some(p) = c.parent() {
            prop_assert!(p.children().unwrap().contains(&c));
        } else {
            prop_assert_eq!(c.level, 0);
        }
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p wb-grid --test cell`
Expected: FAIL — unresolved imports `CellId`, `TileId`, etc.

- [ ] **Step 3: Implement**

`crates/wb-grid/src/cell.rs`:
```rust
use crate::face::{face_to_sphere, sphere_to_face, Face};
use crate::vec3::Vec3;

pub const TILE_SIZE: u32 = 256;
pub const MAX_LEVEL: u8 = 12;

pub const fn tiles_per_edge(level: u8) -> u32 {
    1u32 << level
}

pub const fn cells_per_edge(level: u8) -> u32 {
    TILE_SIZE << level
}

/// A 256×256 tile. `x` runs along the face's u axis, `y` along v.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileId {
    pub face: Face,
    pub level: u8,
    pub x: u32,
    pub y: u32,
}

/// A single cell, addressed by global per-face indices at a tile level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellId {
    pub face: Face,
    pub level: u8,
    pub i: u32,
    pub j: u32,
}

impl TileId {
    pub fn new(face: Face, level: u8, x: u32, y: u32) -> Option<TileId> {
        let ok = level <= MAX_LEVEL && x < tiles_per_edge(level) && y < tiles_per_edge(level);
        ok.then_some(TileId { face, level, x, y })
    }

    /// Cell at local coordinates `(i, j)`, each in `0..TILE_SIZE`.
    pub fn cell(self, i: u32, j: u32) -> CellId {
        debug_assert!(i < TILE_SIZE && j < TILE_SIZE);
        CellId { face: self.face, level: self.level, i: self.x * TILE_SIZE + i, j: self.y * TILE_SIZE + j }
    }

    pub fn parent(self) -> Option<TileId> {
        (self.level > 0).then(|| TileId { face: self.face, level: self.level - 1, x: self.x / 2, y: self.y / 2 })
    }

    pub fn children(self) -> Option<[TileId; 4]> {
        (self.level < MAX_LEVEL).then(|| {
            let (face, level, x, y) = (self.face, self.level + 1, self.x * 2, self.y * 2);
            [
                TileId { face, level, x, y },
                TileId { face, level, x: x + 1, y },
                TileId { face, level, x, y: y + 1 },
                TileId { face, level, x: x + 1, y: y + 1 },
            ]
        })
    }

    /// Every tile at `level`, in `Ord` order (face, level, x, y).
    pub fn all(level: u8) -> Vec<TileId> {
        let n = tiles_per_edge(level);
        let mut out = Vec::with_capacity(6 * (n * n) as usize);
        for face in Face::ALL {
            for x in 0..n {
                for y in 0..n {
                    out.push(TileId { face, level, x, y });
                }
            }
        }
        out
    }
}

impl CellId {
    pub fn new(face: Face, level: u8, i: u32, j: u32) -> Option<CellId> {
        let ok = level <= MAX_LEVEL && i < cells_per_edge(level) && j < cells_per_edge(level);
        ok.then_some(CellId { face, level, i, j })
    }

    /// Equi-angular coordinates of the cell centre.
    pub fn center_st(self) -> (f64, f64) {
        let m = cells_per_edge(self.level) as f64;
        (-1.0 + (self.i as f64 + 0.5) * 2.0 / m, -1.0 + (self.j as f64 + 0.5) * 2.0 / m)
    }

    pub fn center(self) -> Vec3 {
        let (s, t) = self.center_st();
        face_to_sphere(self.face, s, t)
    }

    /// The cell containing direction `p` at `level`.
    pub fn locate(p: Vec3, level: u8) -> CellId {
        let (face, s, t) = sphere_to_face(p);
        let m = cells_per_edge(level);
        let index = |c: f64| -> u32 {
            let k = ((c + 1.0) * 0.5 * m as f64) as i64;
            k.clamp(0, m as i64 - 1) as u32
        };
        CellId { face, level, i: index(s), j: index(t) }
    }

    /// `(tile, local_i, local_j)`.
    pub fn tile(self) -> (TileId, u32, u32) {
        let tile = TileId { face: self.face, level: self.level, x: self.i / TILE_SIZE, y: self.j / TILE_SIZE };
        (tile, self.i % TILE_SIZE, self.j % TILE_SIZE)
    }

    pub fn parent(self) -> Option<CellId> {
        (self.level > 0).then(|| CellId { face: self.face, level: self.level - 1, i: self.i / 2, j: self.j / 2 })
    }

    pub fn children(self) -> Option<[CellId; 4]> {
        (self.level < MAX_LEVEL).then(|| {
            let (face, level, i, j) = (self.face, self.level + 1, self.i * 2, self.j * 2);
            [
                CellId { face, level, i, j },
                CellId { face, level, i: i + 1, j },
                CellId { face, level, i, j: j + 1 },
                CellId { face, level, i: i + 1, j: j + 1 },
            ]
        })
    }
}
```

Add to `crates/wb-grid/src/lib.rs`:
```rust
mod cell;
pub use cell::{cells_per_edge, tiles_per_edge, CellId, TileId, MAX_LEVEL, TILE_SIZE};
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p wb-grid --test cell`
Expected: PASS (7 tests).

- [ ] **Step 5: Gate and commit**

Run: `scripts/check.sh` — expected all green.
```bash
git add crates/wb-grid
git commit -m "feat(grid): tile and cell indexing with quadtree hierarchy" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Cross-face neighbours

**Files:**
- Create: `crates/wb-grid/src/neighbors.rs`
- Modify: `crates/wb-grid/src/lib.rs`
- Test: `crates/wb-grid/tests/neighbors.rs`

**Interfaces:**
- Consumes: `CellId`, `cells_per_edge`, `face_to_sphere` (Tasks 3–4).
- Produces:
  - `pub const DIRS4: [(i32, i32); 4]` and `pub const DIRS8: [(i32, i32); 8]`.
  - `CellId::offset(self, di: i32, dj: i32) -> CellId` — `di, dj ∈ {-1, 0, 1}`; crosses face edges correctly.
  - `CellId::neighbors4(self) -> [CellId; 4]` in `DIRS4` order.
  - `CellId::neighbors8(self) -> Vec<CellId>` — distinct, excludes self; length 8, or 7 at the 24 cube-corner cells.

- [ ] **Step 1: Write the failing tests**

`crates/wb-grid/tests/neighbors.rs`:
```rust
use core::f64::consts::FRAC_PI_2;
use wb_grid::{cells_per_edge, CellId, Face};

fn edge_cells(level: u8) -> Vec<CellId> {
    let m = cells_per_edge(level);
    let mut out = Vec::new();
    for face in Face::ALL {
        for k in 0..m {
            for (i, j) in [(k, 0), (k, m - 1), (0, k), (m - 1, k)] {
                out.push(CellId::new(face, level, i, j).unwrap());
            }
        }
    }
    out
}

#[test]
fn neighbours4_are_symmetric_across_every_face_edge() {
    for c in edge_cells(0) {
        for n in c.neighbors4() {
            assert_ne!(n, c);
            assert!(n.neighbors4().contains(&c), "{c:?} -> {n:?} is not mutual");
        }
    }
}

#[test]
fn neighbours4_are_close() {
    let m = cells_per_edge(0) as f64;
    let max_angle = 1.5 * FRAC_PI_2 / m;
    for c in edge_cells(0) {
        for n in c.neighbors4() {
            assert!(c.center().dot(n.center()) > libm::cos(max_angle), "{c:?} -> {n:?}");
        }
    }
}

#[test]
fn neighbours4_cross_to_the_expected_face() {
    let m = cells_per_edge(0);
    let c = CellId::new(Face::PosX, 0, m - 1, 100).unwrap();
    // +u on PosX points toward +Y.
    assert_eq!(c.offset(1, 0).face, Face::PosY);
}

#[test]
fn neighbours8_counts() {
    let m = cells_per_edge(0);
    for c in edge_cells(0) {
        let n8 = c.neighbors8();
        let corner = (c.i == 0 || c.i == m - 1) && (c.j == 0 || c.j == m - 1);
        assert_eq!(n8.len(), if corner { 7 } else { 8 }, "{c:?}");
        assert!(!n8.contains(&c));
    }
    let interior = CellId::new(Face::NegZ, 0, 10, 10).unwrap();
    assert_eq!(interior.neighbors8().len(), 8);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p wb-grid --test neighbors`
Expected: FAIL — `no method named neighbors4 found for struct CellId`.

- [ ] **Step 3: Implement**

`crates/wb-grid/src/neighbors.rs`:
```rust
use crate::cell::{cells_per_edge, CellId};
use crate::face::face_to_sphere;

pub const DIRS4: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
pub const DIRS8: [(i32, i32); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];

impl CellId {
    /// Neighbouring cell one step away. Off-face steps are resolved by projecting the
    /// would-be cell centre (just past the edge) onto the sphere and locating it.
    pub fn offset(self, di: i32, dj: i32) -> CellId {
        debug_assert!((-1..=1).contains(&di) && (-1..=1).contains(&dj));
        let m = cells_per_edge(self.level) as i64;
        let ni = self.i as i64 + di as i64;
        let nj = self.j as i64 + dj as i64;
        if (0..m).contains(&ni) && (0..m).contains(&nj) {
            return CellId { i: ni as u32, j: nj as u32, ..self };
        }
        let s = -1.0 + (ni as f64 + 0.5) * 2.0 / m as f64;
        let t = -1.0 + (nj as f64 + 0.5) * 2.0 / m as f64;
        CellId::locate(face_to_sphere(self.face, s, t), self.level)
    }

    pub fn neighbors4(self) -> [CellId; 4] {
        DIRS4.map(|(di, dj)| self.offset(di, dj))
    }

    /// Distinct 8-neighbourhood; at cube corners only 7 cells exist.
    pub fn neighbors8(self) -> Vec<CellId> {
        let mut out = Vec::with_capacity(8);
        for (di, dj) in DIRS8 {
            let n = self.offset(di, dj);
            if n != self && !out.contains(&n) {
                out.push(n);
            }
        }
        out
    }
}
```

Add to `crates/wb-grid/src/lib.rs`:
```rust
mod neighbors;
pub use neighbors::{DIRS4, DIRS8};
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p wb-grid --test neighbors`
Expected: PASS (4 tests).

- [ ] **Step 5: Gate and commit**

Run: `scripts/check.sh` — expected all green.
```bash
git add crates/wb-grid
git commit -m "feat(grid): cross-face neighbour lookup" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: Exact cell areas

**Files:**
- Create: `crates/wb-grid/src/area.rs`
- Modify: `crates/wb-grid/src/lib.rs`
- Test: `crates/wb-grid/tests/area.rs`

**Interfaces:**
- Consumes: `CellId`, `cells_per_edge` (Task 4).
- Produces: `CellId::area_unit(self) -> f64` — exact solid angle of the cell on the unit sphere (steradians). Children's areas sum to the parent's; all cells sum to 4π.

- [ ] **Step 1: Write the failing tests**

`crates/wb-grid/tests/area.rs`:
```rust
use core::f64::consts::PI;
use wb_grid::{cells_per_edge, CellId, Face};

#[test]
fn level0_cells_tile_the_sphere() {
    let m = cells_per_edge(0);
    let mut total = 0.0;
    for face in Face::ALL {
        for j in 0..m {
            for i in 0..m {
                total += CellId::new(face, 0, i, j).unwrap().area_unit();
            }
        }
    }
    assert!((total - 4.0 * PI).abs() < 1e-9 * 4.0 * PI, "total = {total}");
}

#[test]
fn children_sum_to_parent() {
    for (i, j) in [(0, 0), (17, 200), (128, 128), (255, 3)] {
        let p = CellId::new(Face::PosZ, 1, i, j).unwrap();
        let sum: f64 = p.children().unwrap().iter().map(|c| c.area_unit()).sum();
        assert!((sum - p.area_unit()).abs() < 1e-15, "{p:?}");
    }
}

#[test]
fn equi_angular_area_ratio_is_bounded() {
    let m = cells_per_edge(0);
    let corner = CellId::new(Face::PosX, 0, 0, 0).unwrap().area_unit();
    let centre = CellId::new(Face::PosX, 0, m / 2, m / 2).unwrap().area_unit();
    let ratio = corner.max(centre) / corner.min(centre);
    assert!(ratio > 1.0 && ratio < 1.5, "ratio = {ratio}");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p wb-grid --test area`
Expected: FAIL — `no method named area_unit found for struct CellId`.

- [ ] **Step 3: Implement**

`crates/wb-grid/src/area.rs`:
```rust
use crate::cell::{cells_per_edge, CellId};
use core::f64::consts::FRAC_PI_4;

/// Solid angle of the gnomonic rectangle [0, a] × [0, b] on a unit-distance plane.
fn rect_solid_angle(a: f64, b: f64) -> f64 {
    libm::atan(a * b / (1.0 + a * a + b * b).sqrt())
}

impl CellId {
    /// Exact area of this cell on the unit sphere, in steradians.
    pub fn area_unit(self) -> f64 {
        let m = cells_per_edge(self.level) as f64;
        let edge = |k: u32| libm::tan(FRAC_PI_4 * (-1.0 + k as f64 * 2.0 / m));
        let (a0, a1) = (edge(self.i), edge(self.i + 1));
        let (b0, b1) = (edge(self.j), edge(self.j + 1));
        rect_solid_angle(a1, b1) - rect_solid_angle(a0, b1) - rect_solid_angle(a1, b0)
            + rect_solid_angle(a0, b0)
    }
}
```

Add to `crates/wb-grid/src/lib.rs`:
```rust
mod area;
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p wb-grid --test area`
Expected: PASS (3 tests).

- [ ] **Step 5: Gate and commit**

Run: `scripts/check.sh` — expected all green, including all `wb-grid` tests under wasm32-wasip1.
```bash
git add crates/wb-grid
git commit -m "feat(grid): exact spherical cell areas" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 7: Typed tiles with stable encoding

**Files:**
- Create: `crates/wb-world/Cargo.toml`, `crates/wb-world/src/lib.rs`, `crates/wb-world/src/tile.rs`
- Test: `crates/wb-world/tests/tile.rs`

**Interfaces:**
- Consumes: `TileId`, `Face`, `TILE_SIZE` (wb-grid).
- Produces:
  - `pub const TILE_CELLS: usize = 65_536;`
  - `#[repr(u8)] pub enum Dtype { F32 = 0, U8 = 1, U16 = 2, I16 = 3 }` (derives `Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash`).
  - `pub trait CellValue: Copy + Default + PartialEq + Debug + 'static { const DTYPE: Dtype; const BYTES: usize; fn write_le(self, out: &mut Vec<u8>); fn read_le(bytes: &[u8]) -> Self; }` implemented for `f32, u8, u16, i16`.
  - `pub struct Tile<T: CellValue>` — `new(TileId) -> Tile<T>` (all default), `from_fn(TileId, impl FnMut(u32, u32) -> T) -> Tile<T>`, `id(&self) -> TileId`, `get(&self, i, j) -> T`, `set(&mut self, i, j, T)`, `data(&self) -> &[T]`, `encode(&self) -> Vec<u8>`, `decode(&[u8]) -> Result<Tile<T>, DecodeError>`, `content_hash(&self) -> [u8; 32]`.
  - `pub enum DecodeError { BadMagic, WrongDtype, BadHeader, BadLength }` (implements `Display` + `Error`).
  - Encoding: `b"WBT1"`, dtype `u8`, face `u8`, level `u8`, `0u8`, `x: u32 LE`, `y: u32 LE`, then cells row-major little-endian.

- [ ] **Step 1: Create the crate manifest**

`crates/wb-world/Cargo.toml`:
```toml
[package]
name = "wb-world"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
wb-grid.workspace = true
libm.workspace = true
blake3.workspace = true

[dev-dependencies]
proptest.workspace = true

[lints]
workspace = true
```

`crates/wb-world/src/lib.rs`:
```rust
//! World data model: typed per-cell layers in tiles, LOD, extents, features, hashing.
```

- [ ] **Step 2: Write the failing tests**

`crates/wb-world/tests/tile.rs`:
```rust
use wb_grid::{Face, TileId};
use wb_world::{DecodeError, Tile, TILE_CELLS};

fn tid() -> TileId {
    TileId::new(Face::PosY, 2, 1, 3).unwrap()
}

#[test]
fn from_fn_is_row_major() {
    let t: Tile<u16> = Tile::from_fn(tid(), |i, j| (j * 256 + i) as u16);
    assert_eq!(t.data().len(), TILE_CELLS);
    assert_eq!(t.get(5, 0), 5);
    assert_eq!(t.get(0, 1), 256);
    assert_eq!(t.data()[256 * 7 + 9], t.get(9, 7));
}

#[test]
fn encode_decode_roundtrip() {
    let t: Tile<f32> = Tile::from_fn(tid(), |i, j| i as f32 * 0.5 - j as f32);
    let bytes = t.encode();
    assert_eq!(&bytes[..4], b"WBT1");
    assert_eq!(bytes.len(), 16 + TILE_CELLS * 4);
    assert_eq!(Tile::<f32>::decode(&bytes).unwrap(), t);
}

#[test]
fn decode_rejects_bad_input() {
    let t: Tile<i16> = Tile::new(tid());
    let bytes = t.encode();
    assert_eq!(Tile::<u16>::decode(&bytes).unwrap_err(), DecodeError::WrongDtype);
    assert_eq!(Tile::<i16>::decode(&bytes[..100]).unwrap_err(), DecodeError::BadLength);
    let mut bad = bytes.clone();
    bad[0] = b'X';
    assert_eq!(Tile::<i16>::decode(&bad).unwrap_err(), DecodeError::BadMagic);
}

#[test]
fn content_hash_tracks_content() {
    let mut t: Tile<u8> = Tile::new(tid());
    let h0 = t.content_hash();
    assert_eq!(h0, Tile::<u8>::new(tid()).content_hash());
    t.set(3, 4, 9);
    assert_ne!(t.content_hash(), h0);
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p wb-world --test tile`
Expected: FAIL — unresolved imports `DecodeError`, `Tile`, `TILE_CELLS`.

- [ ] **Step 4: Implement**

`crates/wb-world/src/tile.rs`:
```rust
use core::fmt;
use wb_grid::{Face, TileId, TILE_SIZE};

pub const TILE_CELLS: usize = (TILE_SIZE * TILE_SIZE) as usize;

const MAGIC: &[u8; 4] = b"WBT1";
const HEADER: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Dtype {
    F32 = 0,
    U8 = 1,
    U16 = 2,
    I16 = 3,
}

/// A value storable in a tile, with a fixed little-endian encoding.
pub trait CellValue: Copy + Default + PartialEq + fmt::Debug + 'static {
    const DTYPE: Dtype;
    const BYTES: usize;
    fn write_le(self, out: &mut Vec<u8>);
    fn read_le(bytes: &[u8]) -> Self;
}

macro_rules! cell_value {
    ($t:ty, $dtype:expr, $n:expr) => {
        impl CellValue for $t {
            const DTYPE: Dtype = $dtype;
            const BYTES: usize = $n;
            fn write_le(self, out: &mut Vec<u8>) {
                out.extend_from_slice(&self.to_le_bytes());
            }
            fn read_le(bytes: &[u8]) -> Self {
                let mut a = [0u8; $n];
                a.copy_from_slice(&bytes[..$n]);
                <$t>::from_le_bytes(a)
            }
        }
    };
}

cell_value!(f32, Dtype::F32, 4);
cell_value!(u8, Dtype::U8, 1);
cell_value!(u16, Dtype::U16, 2);
cell_value!(i16, Dtype::I16, 2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    BadMagic,
    WrongDtype,
    BadHeader,
    BadLength,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            DecodeError::BadMagic => "not a WBT1 tile",
            DecodeError::WrongDtype => "tile dtype does not match the requested type",
            DecodeError::BadHeader => "tile header has an invalid face, level, or position",
            DecodeError::BadLength => "tile payload has the wrong length",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for DecodeError {}

/// 256×256 cells of one layer, row-major (`j * 256 + i`).
#[derive(Clone, Debug, PartialEq)]
pub struct Tile<T: CellValue> {
    id: TileId,
    data: Vec<T>,
}

impl<T: CellValue> Tile<T> {
    pub fn new(id: TileId) -> Self {
        Self { id, data: vec![T::default(); TILE_CELLS] }
    }

    pub fn from_fn(id: TileId, mut f: impl FnMut(u32, u32) -> T) -> Self {
        let mut data = Vec::with_capacity(TILE_CELLS);
        for j in 0..TILE_SIZE {
            for i in 0..TILE_SIZE {
                data.push(f(i, j));
            }
        }
        Self { id, data }
    }

    pub fn id(&self) -> TileId {
        self.id
    }

    pub fn get(&self, i: u32, j: u32) -> T {
        self.data[(j * TILE_SIZE + i) as usize]
    }

    pub fn set(&mut self, i: u32, j: u32, v: T) {
        self.data[(j * TILE_SIZE + i) as usize] = v;
    }

    pub fn data(&self) -> &[T] {
        &self.data
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER + TILE_CELLS * T::BYTES);
        out.extend_from_slice(MAGIC);
        out.push(T::DTYPE as u8);
        out.push(self.id.face.index());
        out.push(self.id.level);
        out.push(0);
        out.extend_from_slice(&self.id.x.to_le_bytes());
        out.extend_from_slice(&self.id.y.to_le_bytes());
        for v in &self.data {
            v.write_le(&mut out);
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < HEADER || &bytes[..4] != MAGIC {
            return Err(DecodeError::BadMagic);
        }
        if bytes[4] != T::DTYPE as u8 {
            return Err(DecodeError::WrongDtype);
        }
        let face = Face::from_index(bytes[5]).ok_or(DecodeError::BadHeader)?;
        let x = u32::from_le_bytes(bytes[8..12].try_into().expect("4 bytes"));
        let y = u32::from_le_bytes(bytes[12..16].try_into().expect("4 bytes"));
        let id = TileId::new(face, bytes[6], x, y).ok_or(DecodeError::BadHeader)?;
        if bytes.len() != HEADER + TILE_CELLS * T::BYTES {
            return Err(DecodeError::BadLength);
        }
        let data = bytes[HEADER..].chunks_exact(T::BYTES).map(T::read_le).collect();
        Ok(Self { id, data })
    }

    pub fn content_hash(&self) -> [u8; 32] {
        blake3::hash(&self.encode()).into()
    }
}
```

`crates/wb-world/src/lib.rs` becomes:
```rust
//! World data model: typed per-cell layers in tiles, LOD, extents, features, hashing.

mod tile;

pub use tile::{CellValue, DecodeError, Dtype, Tile, TILE_CELLS};
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p wb-world --test tile`
Expected: PASS (4 tests).

- [ ] **Step 6: Gate and commit**

Run: `scripts/check.sh` — expected all green.
```bash
git add crates/wb-world Cargo.lock
git commit -m "feat(world): typed tiles with stable binary encoding and content hash" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 8: Project extents

**Files:**
- Create: `crates/wb-world/src/extent.rs`
- Modify: `crates/wb-world/src/lib.rs`
- Test: `crates/wb-world/tests/extent.rs`

**Interfaces:**
- Consumes: `LatLon`, `CellId`, `TileId`, `face_to_sphere`, `tiles_per_edge`, `MAX_LEVEL` (wb-grid).
- Produces:
  - `pub struct RegionBox { pub south: f64, pub north: f64, pub west: f64, pub east: f64 }` (radians; `west > east` means the box crosses the antimeridian) — `new(south, north, west, east) -> Result<RegionBox, ExtentError>`, `from_degrees(...) -> Result<RegionBox, ExtentError>`, `contains(&self, LatLon) -> bool`, `sample_points(&self) -> [LatLon; 9]`.
  - `pub enum Extent { Planet, Region(RegionBox) }` — `contains(&self, LatLon) -> bool`, `tiles(&self, level: u8) -> Vec<TileId>` (sorted; panics if `level > MAX_LEVEL`).
  - `pub enum ExtentError { LatitudeOrder, LatitudeRange, LongitudeRange, ZeroWidth }` (`Display` + `Error`).
  - Region tile selection is conservative: a tile is included if any of a 17×17 lattice of points on it lies in the box, or if any of the box's 9 sample points falls in it; selection descends the quadtree from level 0. Slivers thinner than 1/16 of a tile along the box border may be missed; that is acceptable because region borders are open boundaries.

- [ ] **Step 1: Write the failing tests**

`crates/wb-world/tests/extent.rs`:
```rust
use wb_grid::{Face, LatLon, TileId};
use wb_world::{Extent, ExtentError, RegionBox};

#[test]
fn planet_covers_all_tiles() {
    assert_eq!(Extent::Planet.tiles(1), TileId::all(1));
    assert!(Extent::Planet.contains(LatLon::from_degrees(-80.0, 170.0)));
}

#[test]
fn validation() {
    assert_eq!(RegionBox::from_degrees(10.0, 5.0, 0.0, 1.0).unwrap_err(), ExtentError::LatitudeOrder);
    assert_eq!(RegionBox::from_degrees(-95.0, 5.0, 0.0, 1.0).unwrap_err(), ExtentError::LatitudeRange);
    assert_eq!(RegionBox::from_degrees(0.0, 5.0, 0.0, 190.0).unwrap_err(), ExtentError::LongitudeRange);
    assert_eq!(RegionBox::from_degrees(0.0, 5.0, 7.0, 7.0).unwrap_err(), ExtentError::ZeroWidth);
}

#[test]
fn antimeridian_box() {
    let r = RegionBox::from_degrees(-10.0, 10.0, 170.0, -170.0).unwrap();
    assert!(r.contains(LatLon::from_degrees(0.0, 180.0)));
    assert!(r.contains(LatLon::from_degrees(0.0, -175.0)));
    assert!(!r.contains(LatLon::from_degrees(0.0, 0.0)));
}

#[test]
fn polar_cap_region_tiles() {
    let r = Extent::Region(RegionBox::from_degrees(60.0, 70.0, -10.0, 10.0).unwrap());
    let f = Face::PosZ;
    assert_eq!(r.tiles(0), vec![TileId::new(f, 0, 0, 0).unwrap()]);
    assert_eq!(r.tiles(1), vec![TileId::new(f, 1, 0, 0).unwrap(), TileId::new(f, 1, 1, 0).unwrap()]);
}

#[test]
fn northern_hemisphere_excludes_south_face() {
    let r = Extent::Region(RegionBox::from_degrees(0.0, 90.0, -180.0, 180.0).unwrap());
    let faces: Vec<Face> = r.tiles(0).iter().map(|t| t.face).collect();
    assert_eq!(faces, vec![Face::PosX, Face::NegX, Face::PosY, Face::NegY, Face::PosZ]);
}

#[test]
fn tiny_region_inside_one_tile_is_found() {
    let r = Extent::Region(RegionBox::from_degrees(12.0, 12.01, 33.0, 33.01).unwrap());
    let tiles = r.tiles(5);
    assert!(!tiles.is_empty() && tiles.len() <= 4, "{tiles:?}");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p wb-world --test extent`
Expected: FAIL — unresolved imports `Extent`, `ExtentError`, `RegionBox`.

- [ ] **Step 3: Implement**

`crates/wb-world/src/extent.rs`:
```rust
use core::f64::consts::{FRAC_PI_2, PI};
use core::fmt;
use std::collections::BTreeSet;
use wb_grid::{face_to_sphere, tiles_per_edge, CellId, LatLon, TileId, MAX_LEVEL};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtentError {
    LatitudeOrder,
    LatitudeRange,
    LongitudeRange,
    ZeroWidth,
}

impl fmt::Display for ExtentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            ExtentError::LatitudeOrder => "south must be less than north",
            ExtentError::LatitudeRange => "latitudes must be within ±90°",
            ExtentError::LongitudeRange => "longitudes must be within ±180°",
            ExtentError::ZeroWidth => "west and east must differ",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for ExtentError {}

/// Latitude/longitude box in radians. `west > east` crosses the antimeridian.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionBox {
    pub south: f64,
    pub north: f64,
    pub west: f64,
    pub east: f64,
}

fn wrap_lon(x: f64) -> f64 {
    if x > PI { x - 2.0 * PI } else if x <= -PI { x + 2.0 * PI } else { x }
}

impl RegionBox {
    pub fn new(south: f64, north: f64, west: f64, east: f64) -> Result<Self, ExtentError> {
        if !(-FRAC_PI_2..=FRAC_PI_2).contains(&south) || !(-FRAC_PI_2..=FRAC_PI_2).contains(&north) {
            return Err(ExtentError::LatitudeRange);
        }
        if south >= north {
            return Err(ExtentError::LatitudeOrder);
        }
        if !(-PI..=PI).contains(&west) || !(-PI..=PI).contains(&east) {
            return Err(ExtentError::LongitudeRange);
        }
        if west == east {
            return Err(ExtentError::ZeroWidth);
        }
        Ok(Self { south, north, west, east })
    }

    pub fn from_degrees(south: f64, north: f64, west: f64, east: f64) -> Result<Self, ExtentError> {
        Self::new(south.to_radians(), north.to_radians(), west.to_radians(), east.to_radians())
    }

    pub fn contains(&self, p: LatLon) -> bool {
        if p.lat < self.south || p.lat > self.north {
            return false;
        }
        if self.west <= self.east {
            p.lon >= self.west && p.lon <= self.east
        } else {
            p.lon >= self.west || p.lon <= self.east
        }
    }

    /// Corners, edge midpoints, and centre.
    pub fn sample_points(&self) -> [LatLon; 9] {
        let span = if self.west <= self.east { self.east - self.west } else { self.east - self.west + 2.0 * PI };
        let lons = [self.west, wrap_lon(self.west + span / 2.0), self.east];
        let lats = [self.south, (self.south + self.north) / 2.0, self.north];
        let mut out = [LatLon { lat: 0.0, lon: 0.0 }; 9];
        for (k, (lat, lon)) in lats.iter().flat_map(|a| lons.iter().map(move |o| (*a, *o))).enumerate() {
            out[k] = LatLon { lat, lon };
        }
        out
    }

    fn touches(&self, t: TileId) -> bool {
        const K: u32 = 16;
        let n = tiles_per_edge(t.level) as f64;
        (0..=K).any(|a| {
            (0..=K).any(|b| {
                let s = -1.0 + 2.0 * (t.x as f64 + a as f64 / K as f64) / n;
                let tt = -1.0 + 2.0 * (t.y as f64 + b as f64 / K as f64) / n;
                self.contains(LatLon::from_vec3(face_to_sphere(t.face, s, tt)))
            })
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Extent {
    Planet,
    Region(RegionBox),
}

impl Extent {
    pub fn contains(&self, p: LatLon) -> bool {
        match self {
            Extent::Planet => true,
            Extent::Region(r) => r.contains(p),
        }
    }

    /// Tiles at `level` covering this extent, sorted.
    pub fn tiles(&self, level: u8) -> Vec<TileId> {
        assert!(level <= MAX_LEVEL, "level {level} exceeds MAX_LEVEL");
        let r = match self {
            Extent::Planet => return TileId::all(level),
            Extent::Region(r) => r,
        };
        let samples = r.sample_points();
        let located = |l: u8| samples.map(|p| CellId::locate(p.to_vec3(), l).tile().0);
        let mut current: BTreeSet<TileId> = TileId::all(0).into_iter().filter(|t| r.touches(*t)).collect();
        current.extend(located(0));
        for l in 1..=level {
            let mut next = BTreeSet::new();
            for t in &current {
                for c in t.children().expect("level <= MAX_LEVEL") {
                    if r.touches(c) {
                        next.insert(c);
                    }
                }
            }
            next.extend(located(l));
            current = next;
        }
        current.into_iter().collect()
    }
}
```

Add to `crates/wb-world/src/lib.rs`:
```rust
mod extent;
pub use extent::{Extent, ExtentError, RegionBox};
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p wb-world --test extent`
Expected: PASS (6 tests).

- [ ] **Step 5: Gate and commit**

Run: `scripts/check.sh` — expected all green.
```bash
git add crates/wb-world
git commit -m "feat(world): whole-planet and regional extents with tile selection" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 9: Layer registry, planet, and world store

**Files:**
- Create: `crates/wb-world/src/layer.rs`, `crates/wb-world/src/world.rs`
- Modify: `crates/wb-world/src/lib.rs`
- Test: `crates/wb-world/tests/world.rs`

**Interfaces:**
- Consumes: `Tile`, `CellValue`, `Dtype` (Task 7); `Extent` (Task 8); `CellId`, `TileId` (wb-grid).
- Produces:
  - `pub struct LayerId(pub u16)` (derives `Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash`).
  - `#[repr(u8)] pub enum LodPolicy { AreaMean = 0, Mode = 1, Max = 2 }`.
  - `pub struct LayerDesc { pub name: String, pub dtype: Dtype, pub unit: String, pub producer: String, pub lod: LodPolicy }`.
  - `pub struct LayerRegistry` — `register(&mut self, LayerDesc) -> Result<LayerId, RegistryError>`, `get(&self, LayerId) -> Option<&LayerDesc>`, `id_of(&self, &str) -> Option<LayerId>`, `iter(&self) -> impl Iterator<Item = (LayerId, &LayerDesc)>` (ids in registration order starting at 0).
  - `pub enum RegistryError { DuplicateName(String), PolicyNotSupported { name: String, dtype: Dtype } }` — `AreaMean` is only valid for `F32`.
  - `pub const ENGINE_VERSION: &str` (= crate version).
  - `pub struct Planet { pub radius_m: f64 }` — `Default` = 6 371 000 m; `cell_area_m2(&self, CellId) -> f64`.
  - `pub struct LayerStore<T: CellValue>` (`Default`) — `insert(&mut self, Tile<T>) -> Option<Tile<T>>`, `get(&self, TileId) -> Option<&Tile<T>>`, `get_mut(&mut self, TileId) -> Option<&mut Tile<T>>`, `cell(&self, CellId) -> Option<T>`, `iter(&self) -> impl Iterator<Item = (&TileId, &Tile<T>)>` (sorted), `len`, `is_empty`.
  - `pub trait WorldValue: CellValue` implemented for `f32, u8, u16, i16`.
  - `pub struct World { pub planet: Planet, pub extent: Extent, … }` — `new(Planet, Extent) -> World`, `registry(&self) -> &LayerRegistry`, `register_layer(&mut self, LayerDesc) -> Result<LayerId, WorldError>`, `layer<T: WorldValue>(&self, LayerId) -> Result<&LayerStore<T>, WorldError>`, `layer_mut<T: WorldValue>(&mut self, LayerId) -> Result<&mut LayerStore<T>, WorldError>`.
  - `pub enum WorldError { Registry(RegistryError), UnknownLayer(LayerId), DtypeMismatch { layer: Dtype, requested: Dtype } }`.

- [ ] **Step 1: Write the failing tests**

`crates/wb-world/tests/world.rs`:
```rust
use core::f64::consts::PI;
use wb_grid::{cells_per_edge, CellId, Face, TileId};
use wb_world::{
    Dtype, Extent, LayerDesc, LodPolicy, Planet, RegistryError, Tile, World, WorldError, ENGINE_VERSION,
};

fn elevation() -> LayerDesc {
    LayerDesc {
        name: "elevation".into(),
        dtype: Dtype::F32,
        unit: "m".into(),
        producer: "test".into(),
        lod: LodPolicy::AreaMean,
    }
}

#[test]
fn engine_version_is_set() {
    assert!(!ENGINE_VERSION.is_empty());
}

#[test]
fn planet_cell_areas_sum_to_sphere_area() {
    let planet = Planet::default();
    assert_eq!(planet.radius_m, 6_371_000.0);
    let m = cells_per_edge(0);
    let face_area: f64 = (0..m)
        .flat_map(|j| (0..m).map(move |i| (i, j)))
        .map(|(i, j)| planet.cell_area_m2(CellId::new(Face::PosX, 0, i, j).unwrap()))
        .sum();
    let sphere = 4.0 * PI * planet.radius_m * planet.radius_m;
    assert!((face_area * 6.0 - sphere).abs() < 1e-8 * sphere);
}

#[test]
fn registry_rules() {
    let mut w = World::new(Planet::default(), Extent::Planet);
    let id = w.register_layer(elevation()).unwrap();
    assert_eq!(w.registry().id_of("elevation"), Some(id));
    assert_eq!(w.registry().get(id).unwrap().unit, "m");
    assert_eq!(
        w.register_layer(elevation()).unwrap_err(),
        WorldError::Registry(RegistryError::DuplicateName("elevation".into()))
    );
    let bad = LayerDesc { name: "biome".into(), dtype: Dtype::U8, lod: LodPolicy::AreaMean, ..elevation() };
    assert_eq!(
        w.register_layer(bad).unwrap_err(),
        WorldError::Registry(RegistryError::PolicyNotSupported { name: "biome".into(), dtype: Dtype::U8 })
    );
}

#[test]
fn typed_layer_access() {
    let mut w = World::new(Planet::default(), Extent::Planet);
    let id = w.register_layer(elevation()).unwrap();
    let tid = TileId::new(Face::NegX, 0, 0, 0).unwrap();
    w.layer_mut::<f32>(id).unwrap().insert(Tile::from_fn(tid, |i, _| i as f32));
    let store = w.layer::<f32>(id).unwrap();
    assert_eq!(store.len(), 1);
    assert_eq!(store.cell(tid.cell(7, 3)), Some(7.0));
    assert_eq!(store.cell(TileId::new(Face::PosX, 0, 0, 0).unwrap().cell(0, 0)), None);
    assert_eq!(
        w.layer::<u8>(id).unwrap_err(),
        WorldError::DtypeMismatch { layer: Dtype::F32, requested: Dtype::U8 }
    );
    assert_eq!(w.layer::<f32>(wb_world::LayerId(99)).unwrap_err(), WorldError::UnknownLayer(wb_world::LayerId(99)));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p wb-world --test world`
Expected: FAIL — unresolved imports `LayerDesc`, `World`, etc.

- [ ] **Step 3: Implement the registry**

`crates/wb-world/src/layer.rs`:
```rust
use crate::tile::Dtype;
use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerId(pub u16);

/// How a parent cell's value is derived from its 4 children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LodPolicy {
    /// Area-weighted mean ("coarse is truth"); F32 only.
    AreaMean = 0,
    /// Most common value (categorical layers such as biome).
    Mode = 1,
    /// Maximum value.
    Max = 2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayerDesc {
    pub name: String,
    pub dtype: Dtype,
    pub unit: String,
    pub producer: String,
    pub lod: LodPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    DuplicateName(String),
    PolicyNotSupported { name: String, dtype: Dtype },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::DuplicateName(n) => write!(f, "layer '{n}' is already registered"),
            RegistryError::PolicyNotSupported { name, dtype } => {
                write!(f, "layer '{name}': AreaMean requires F32, got {dtype:?}")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

#[derive(Clone, Debug, Default)]
pub struct LayerRegistry {
    descs: Vec<LayerDesc>,
}

impl LayerRegistry {
    pub fn register(&mut self, desc: LayerDesc) -> Result<LayerId, RegistryError> {
        if self.id_of(&desc.name).is_some() {
            return Err(RegistryError::DuplicateName(desc.name));
        }
        if desc.lod == LodPolicy::AreaMean && desc.dtype != Dtype::F32 {
            return Err(RegistryError::PolicyNotSupported { name: desc.name, dtype: desc.dtype });
        }
        let id = LayerId(u16::try_from(self.descs.len()).expect("fewer than 65536 layers"));
        self.descs.push(desc);
        Ok(id)
    }

    pub fn get(&self, id: LayerId) -> Option<&LayerDesc> {
        self.descs.get(id.0 as usize)
    }

    pub fn id_of(&self, name: &str) -> Option<LayerId> {
        self.descs.iter().position(|d| d.name == name).map(|k| LayerId(k as u16))
    }

    pub fn iter(&self) -> impl Iterator<Item = (LayerId, &LayerDesc)> {
        self.descs.iter().enumerate().map(|(k, d)| (LayerId(k as u16), d))
    }
}
```

- [ ] **Step 4: Implement the world store**

`crates/wb-world/src/world.rs`:
```rust
use crate::extent::Extent;
use crate::layer::{LayerDesc, LayerId, LayerRegistry, RegistryError};
use crate::tile::{CellValue, Dtype, Tile};
use core::fmt;
use std::collections::BTreeMap;
use wb_grid::{CellId, TileId};

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Planet {
    pub radius_m: f64,
}

impl Default for Planet {
    fn default() -> Self {
        Self { radius_m: 6_371_000.0 }
    }
}

impl Planet {
    pub fn cell_area_m2(&self, c: CellId) -> f64 {
        c.area_unit() * self.radius_m * self.radius_m
    }
}

/// All tiles of one layer, keyed and iterated in `TileId` order.
#[derive(Clone, Debug)]
pub struct LayerStore<T: CellValue> {
    tiles: BTreeMap<TileId, Tile<T>>,
}

impl<T: CellValue> Default for LayerStore<T> {
    fn default() -> Self {
        Self { tiles: BTreeMap::new() }
    }
}

impl<T: CellValue> LayerStore<T> {
    pub fn insert(&mut self, tile: Tile<T>) -> Option<Tile<T>> {
        self.tiles.insert(tile.id(), tile)
    }

    pub fn get(&self, id: TileId) -> Option<&Tile<T>> {
        self.tiles.get(&id)
    }

    pub fn get_mut(&mut self, id: TileId) -> Option<&mut Tile<T>> {
        self.tiles.get_mut(&id)
    }

    pub fn cell(&self, c: CellId) -> Option<T> {
        let (tile, i, j) = c.tile();
        self.tiles.get(&tile).map(|t| t.get(i, j))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&TileId, &Tile<T>)> {
        self.tiles.iter()
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorldError {
    Registry(RegistryError),
    UnknownLayer(LayerId),
    DtypeMismatch { layer: Dtype, requested: Dtype },
}

impl fmt::Display for WorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorldError::Registry(e) => write!(f, "{e}"),
            WorldError::UnknownLayer(id) => write!(f, "unknown layer {id:?}"),
            WorldError::DtypeMismatch { layer, requested } => {
                write!(f, "layer holds {layer:?} but {requested:?} was requested")
            }
        }
    }
}

impl std::error::Error for WorldError {}

/// The generated world: planet, extent, and typed per-cell layers.
#[derive(Clone, Debug)]
pub struct World {
    pub planet: Planet,
    pub extent: Extent,
    registry: LayerRegistry,
    f32_layers: BTreeMap<LayerId, LayerStore<f32>>,
    u8_layers: BTreeMap<LayerId, LayerStore<u8>>,
    u16_layers: BTreeMap<LayerId, LayerStore<u16>>,
    i16_layers: BTreeMap<LayerId, LayerStore<i16>>,
}

/// Cell types a `World` can store.
pub trait WorldValue: CellValue {
    #[doc(hidden)]
    fn stores(w: &World) -> &BTreeMap<LayerId, LayerStore<Self>>;
    #[doc(hidden)]
    fn stores_mut(w: &mut World) -> &mut BTreeMap<LayerId, LayerStore<Self>>;
}

macro_rules! world_value {
    ($t:ty, $field:ident) => {
        impl WorldValue for $t {
            fn stores(w: &World) -> &BTreeMap<LayerId, LayerStore<Self>> {
                &w.$field
            }
            fn stores_mut(w: &mut World) -> &mut BTreeMap<LayerId, LayerStore<Self>> {
                &mut w.$field
            }
        }
    };
}

world_value!(f32, f32_layers);
world_value!(u8, u8_layers);
world_value!(u16, u16_layers);
world_value!(i16, i16_layers);

impl World {
    pub fn new(planet: Planet, extent: Extent) -> Self {
        Self {
            planet,
            extent,
            registry: LayerRegistry::default(),
            f32_layers: BTreeMap::new(),
            u8_layers: BTreeMap::new(),
            u16_layers: BTreeMap::new(),
            i16_layers: BTreeMap::new(),
        }
    }

    pub fn registry(&self) -> &LayerRegistry {
        &self.registry
    }

    /// Registers a layer and creates its empty store.
    pub fn register_layer(&mut self, desc: LayerDesc) -> Result<LayerId, WorldError> {
        let dtype = desc.dtype;
        let id = self.registry.register(desc).map_err(WorldError::Registry)?;
        match dtype {
            Dtype::F32 => {
                self.f32_layers.insert(id, LayerStore::default());
            }
            Dtype::U8 => {
                self.u8_layers.insert(id, LayerStore::default());
            }
            Dtype::U16 => {
                self.u16_layers.insert(id, LayerStore::default());
            }
            Dtype::I16 => {
                self.i16_layers.insert(id, LayerStore::default());
            }
        }
        Ok(id)
    }

    fn check<T: WorldValue>(&self, id: LayerId) -> Result<(), WorldError> {
        let desc = self.registry.get(id).ok_or(WorldError::UnknownLayer(id))?;
        if desc.dtype != T::DTYPE {
            return Err(WorldError::DtypeMismatch { layer: desc.dtype, requested: T::DTYPE });
        }
        Ok(())
    }

    pub fn layer<T: WorldValue>(&self, id: LayerId) -> Result<&LayerStore<T>, WorldError> {
        self.check::<T>(id)?;
        Ok(T::stores(self).get(&id).expect("registered layers always have a store"))
    }

    pub fn layer_mut<T: WorldValue>(&mut self, id: LayerId) -> Result<&mut LayerStore<T>, WorldError> {
        self.check::<T>(id)?;
        Ok(T::stores_mut(self).get_mut(&id).expect("registered layers always have a store"))
    }
}
```

Add to `crates/wb-world/src/lib.rs`:
```rust
mod layer;
mod world;
pub use layer::{LayerDesc, LayerId, LayerRegistry, LodPolicy, RegistryError};
pub use world::{LayerStore, Planet, World, WorldError, WorldValue, ENGINE_VERSION};
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p wb-world --test world`
Expected: PASS (4 tests).

- [ ] **Step 6: Gate and commit**

Run: `scripts/check.sh` — expected all green.
```bash
git add crates/wb-world
git commit -m "feat(world): layer registry, planet, and typed world store" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 10: Level-of-detail operators ("coarse is truth")

**Files:**
- Create: `crates/wb-world/src/lod.rs`
- Modify: `crates/wb-world/src/lib.rs`, `docs/superpowers/specs/2026-09-21-world-builder-architecture-design.md` (§3.3 item 1 wording)
- Test: `crates/wb-world/tests/lod.rs`

**Interfaces:**
- Consumes: `Tile`, `CellValue` (Task 7); `LayerStore` (Task 9); `LodPolicy` (Task 9); `CellId`, `TileId`, `TILE_SIZE` (wb-grid).
- Produces:
  - `pub trait Aggregate: CellValue { fn area_mean(v: [Self; 4], w: [f64; 4]) -> Option<Self>; fn order_key(self) -> u64; }` implemented for `f32` (`area_mean` = weighted mean) and `u8, u16, i16` (`area_mean` = `None`).
  - `pub fn downsample<T: Aggregate>(store: &LayerStore<T>, parent: TileId, policy: LodPolicy) -> Result<Tile<T>, LodError>` — needs all 4 child tiles present.
  - `pub fn refine_cell(parent: f32, detail: [f32; 4], weights: [f64; 4]) -> [f32; 4]` — children = parent + detail − weighted-mean(detail).
  - `pub fn refine_tile(parent_store: &LayerStore<f32>, child: TileId, detail: impl Fn(CellId) -> f32) -> Result<Tile<f32>, LodError>`.
  - `pub enum LodError { MissingTile(TileId), NoParentLevel, NoChildLevel, PolicyNotSupported }` (`Display` + `Error`).
  - Mode tie-break: highest count, then largest total area, then smallest `order_key`.

- [ ] **Step 1: Write the failing tests**

`crates/wb-world/tests/lod.rs`:
```rust
use proptest::prelude::*;
use wb_grid::{CellId, Face, TileId, TILE_SIZE};
use wb_world::{downsample, refine_cell, refine_tile, Aggregate, LayerStore, LodError, LodPolicy, Tile};

const TOL_M: f32 = 2e-3;

fn cfg() -> ProptestConfig {
    ProptestConfig { cases: 256, failure_persistence: None, ..ProptestConfig::default() }
}

proptest! {
    #![proptest_config(cfg())]
    #[test]
    fn refine_preserves_area_weighted_mean(
        parent in -11_000.0f32..9_000.0,
        detail in prop::array::uniform4(-2_000.0f32..2_000.0),
        face in 0u8..6, i in 0u32..512, j in 0u32..512,
    ) {
        let p = CellId::new(Face::from_index(face).unwrap(), 1, i, j).unwrap();
        let w = p.children().unwrap().map(|c| c.area_unit());
        let kids = refine_cell(parent, detail, w);
        let mean = f32::area_mean(kids, w).unwrap();
        prop_assert!((mean - parent).abs() <= TOL_M, "mean {mean} vs parent {parent}");
    }
}

fn detail(c: CellId) -> f32 {
    let (s, t) = c.center_st();
    (300.0 * libm::sin(97.0 * s) * libm::cos(61.0 * t)) as f32
}

#[test]
fn refine_then_downsample_returns_the_parent_tile() {
    let parent_id = TileId::new(Face::PosZ, 0, 0, 0).unwrap();
    let mut parents = LayerStore::<f32>::default();
    parents.insert(Tile::from_fn(parent_id, |i, j| i as f32 * 20.0 - j as f32 * 13.0));
    let mut kids = LayerStore::<f32>::default();
    for c in parent_id.children().unwrap() {
        kids.insert(refine_tile(&parents, c, detail).unwrap());
    }
    let back = downsample(&kids, parent_id, LodPolicy::AreaMean).unwrap();
    let orig = parents.get(parent_id).unwrap();
    for j in 0..TILE_SIZE {
        for i in 0..TILE_SIZE {
            assert!((back.get(i, j) - orig.get(i, j)).abs() <= TOL_M, "cell ({i},{j})");
        }
    }
}

#[test]
fn mode_and_max_policies() {
    let parent = TileId::new(Face::NegY, 0, 0, 0).unwrap();
    let mut kids = LayerStore::<u8>::default();
    for c in parent.children().unwrap() {
        // Within each 2×2 block: three cells of 7 and one of 200.
        kids.insert(Tile::from_fn(c, |i, j| if i % 2 == 1 && j % 2 == 1 { 200 } else { 7 }));
    }
    let mode = downsample(&kids, parent, LodPolicy::Mode).unwrap();
    let max = downsample(&kids, parent, LodPolicy::Max).unwrap();
    assert!(mode.data().iter().all(|v| *v == 7));
    assert!(max.data().iter().all(|v| *v == 200));
    assert_eq!(downsample(&kids, parent, LodPolicy::AreaMean).unwrap_err(), LodError::PolicyNotSupported);
}

#[test]
fn missing_children_are_reported() {
    let parent = TileId::new(Face::PosX, 0, 0, 0).unwrap();
    let store = LayerStore::<f32>::default();
    let first_child = parent.children().unwrap()[0];
    assert_eq!(downsample(&store, parent, LodPolicy::AreaMean).unwrap_err(), LodError::MissingTile(first_child));
    assert_eq!(refine_tile(&store, parent, detail).unwrap_err(), LodError::NoParentLevel);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p wb-world --test lod`
Expected: FAIL — unresolved imports `downsample`, `refine_cell`, …

- [ ] **Step 3: Implement**

`crates/wb-world/src/lod.rs`:
```rust
use crate::layer::LodPolicy;
use crate::tile::{CellValue, Tile};
use crate::world::LayerStore;
use core::fmt;
use wb_grid::{CellId, TileId, TILE_SIZE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LodError {
    MissingTile(TileId),
    NoParentLevel,
    NoChildLevel,
    PolicyNotSupported,
}

impl fmt::Display for LodError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LodError::MissingTile(t) => write!(f, "missing tile {t:?}"),
            LodError::NoParentLevel => f.write_str("level 0 has no parent level"),
            LodError::NoChildLevel => f.write_str("MAX_LEVEL has no child level"),
            LodError::PolicyNotSupported => f.write_str("LOD policy not supported for this cell type"),
        }
    }
}

impl std::error::Error for LodError {}

/// Cell types that can be aggregated from 4 children to 1 parent.
pub trait Aggregate: CellValue {
    /// Area-weighted mean, or `None` if the type is categorical.
    fn area_mean(v: [Self; 4], w: [f64; 4]) -> Option<Self>;
    /// Total-order key used by Mode and Max tie-breaks.
    fn order_key(self) -> u64;
}

impl Aggregate for f32 {
    fn area_mean(v: [f32; 4], w: [f64; 4]) -> Option<f32> {
        let wsum: f64 = w.iter().sum();
        let s: f64 = v.iter().zip(w.iter()).map(|(x, wk)| *x as f64 * wk).sum();
        Some((s / wsum) as f32)
    }
    fn order_key(self) -> u64 {
        let b = self.to_bits();
        (if b >> 31 == 1 { !b } else { b | 0x8000_0000 }) as u64
    }
}

impl Aggregate for u8 {
    fn area_mean(_: [u8; 4], _: [f64; 4]) -> Option<u8> {
        None
    }
    fn order_key(self) -> u64 {
        self as u64
    }
}

impl Aggregate for u16 {
    fn area_mean(_: [u16; 4], _: [f64; 4]) -> Option<u16> {
        None
    }
    fn order_key(self) -> u64 {
        self as u64
    }
}

impl Aggregate for i16 {
    fn area_mean(_: [i16; 4], _: [f64; 4]) -> Option<i16> {
        None
    }
    fn order_key(self) -> u64 {
        (self as i32 + 32_768) as u64
    }
}

fn pick_mode<T: Aggregate>(v: [T; 4], w: [f64; 4]) -> T {
    let stats = |key: u64| -> (u32, f64) {
        v.iter()
            .zip(w.iter())
            .filter(|(x, _)| x.order_key() == key)
            .fold((0, 0.0), |(c, a), (_, wk)| (c + 1, a + wk))
    };
    let mut best = v[0];
    let mut best_stats = stats(best.order_key());
    for &cand in &v[1..] {
        let s = stats(cand.order_key());
        let better = s.0 > best_stats.0
            || (s.0 == best_stats.0
                && (s.1 > best_stats.1 || (s.1 == best_stats.1 && cand.order_key() < best.order_key())));
        if better {
            best = cand;
            best_stats = s;
        }
    }
    best
}

fn pick_max<T: Aggregate>(v: [T; 4]) -> T {
    v.into_iter().fold(v[0], |m, x| if x.order_key() > m.order_key() { x } else { m })
}

/// Parent tile from its 4 child tiles.
pub fn downsample<T: Aggregate>(store: &LayerStore<T>, parent: TileId, policy: LodPolicy) -> Result<Tile<T>, LodError> {
    let kid_ids = parent.children().ok_or(LodError::NoChildLevel)?;
    let kids: Vec<&Tile<T>> = kid_ids
        .iter()
        .map(|k| store.get(*k).ok_or(LodError::MissingTile(*k)))
        .collect::<Result<_, _>>()?;
    let mut out = Tile::new(parent);
    for j in 0..TILE_SIZE {
        for i in 0..TILE_SIZE {
            let cells = parent.cell(i, j).children().ok_or(LodError::NoChildLevel)?;
            let mut v = [T::default(); 4];
            let mut w = [0.0; 4];
            for (k, c) in cells.iter().enumerate() {
                let (t, ci, cj) = c.tile();
                let tile = kids.iter().find(|x| x.id() == t).expect("child cell lies in a child tile");
                v[k] = tile.get(ci, cj);
                w[k] = c.area_unit();
            }
            let value = match policy {
                LodPolicy::AreaMean => T::area_mean(v, w).ok_or(LodError::PolicyNotSupported)?,
                LodPolicy::Mode => pick_mode(v, w),
                LodPolicy::Max => pick_max(v),
            };
            out.set(i, j, value);
        }
    }
    Ok(out)
}

/// Children of one parent: parent + detail − weighted-mean(detail), so the
/// area-weighted mean of the children equals the parent (to f32 rounding).
pub fn refine_cell(parent: f32, detail: [f32; 4], weights: [f64; 4]) -> [f32; 4] {
    let wsum: f64 = weights.iter().sum();
    let dmean: f64 = detail.iter().zip(weights.iter()).map(|(d, w)| *d as f64 * w).sum::<f64>() / wsum;
    detail.map(|d| (parent as f64 + d as f64 - dmean) as f32)
}

/// One child tile from the parent layer plus a detail function.
pub fn refine_tile(
    parent_store: &LayerStore<f32>,
    child: TileId,
    detail: impl Fn(CellId) -> f32,
) -> Result<Tile<f32>, LodError> {
    let parent_id = child.parent().ok_or(LodError::NoParentLevel)?;
    let parent = parent_store.get(parent_id).ok_or(LodError::MissingTile(parent_id))?;
    let mut out = Tile::new(child);
    for j in 0..TILE_SIZE {
        for i in 0..TILE_SIZE {
            let c = child.cell(i, j);
            let p = c.parent().ok_or(LodError::NoParentLevel)?;
            let (_, pi, pj) = p.tile();
            let sibs = p.children().expect("a parent of an existing cell has children");
            let w = sibs.map(|s| s.area_unit());
            let d = sibs.map(&detail);
            let vals = refine_cell(parent.get(pi, pj), d, w);
            let k = sibs.iter().position(|s| *s == c).expect("cell is one of its parent's children");
            out.set(i, j, vals[k]);
        }
    }
    Ok(out)
}
```

Add to `crates/wb-world/src/lib.rs`:
```rust
mod lod;
pub use lod::{downsample, refine_cell, refine_tile, Aggregate, LodError};
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p wb-world --test lod`
Expected: PASS (4 tests).

- [ ] **Step 5: Align the spec wording**

In `docs/superpowers/specs/2026-09-21-world-builder-architecture-design.md` §3.3, replace item 1 with:
```markdown
1. **Coarse is truth** — each 2×2 child block's **area-weighted** mean elevation equals its parent cell's elevation, to f32 rounding (≤ 2 mm for elevations). Refinement adds detail; it never moves features.
```

- [ ] **Step 6: Gate and commit**

Run: `scripts/check.sh` — expected all green.
```bash
git add crates/wb-world docs/superpowers/specs
git commit -m "feat(world): LOD downsample/refine preserving area-weighted mean" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 11: Seam-safe tile aprons

**Files:**
- Create: `crates/wb-world/src/apron.rs`
- Modify: `crates/wb-world/src/lib.rs`, `docs/superpowers/specs/2026-09-21-world-builder-architecture-design.md` (§3.3 item 3 wording)
- Test: `crates/wb-world/tests/apron.rs`

**Interfaces:**
- Consumes: `LayerStore` (Task 9); `Tile`, `CellValue` (Task 7); `CellId::offset` (Task 5); `TileId`, `TILE_SIZE`.
- Produces:
  - `pub const APRON_EDGE: u32 = 258;`
  - `pub fn apron_index(li: i32, lj: i32) -> usize` for `li, lj ∈ -1..=256`.
  - `pub fn gather_with_apron<T: CellValue>(store: &LayerStore<T>, tile: TileId) -> Result<Vec<T>, ApronError>` — the tile plus a 1-cell ring copied from neighbouring tiles (across faces too). At the 4 cube-corner apron positions the value duplicates a 4-neighbour, because only 3 cells meet at a cube corner.
  - `pub enum ApronError { MissingTile(TileId) }` (`Display` + `Error`).

- [ ] **Step 1: Write the failing tests**

`crates/wb-world/tests/apron.rs`:
```rust
use wb_grid::{CellId, Face, TileId, TILE_SIZE};
use wb_world::{apron_index, gather_with_apron, ApronError, LayerStore, Tile, APRON_EDGE};

/// Level-0 store where each cell holds its own global index (exact in f32).
fn indexed_store() -> LayerStore<f32> {
    let mut s = LayerStore::default();
    for t in TileId::all(0) {
        let base = t.face.index() as u32 * 65_536;
        s.insert(Tile::from_fn(t, |i, j| (base + j * 256 + i) as f32));
    }
    s
}

fn decode(v: f32) -> CellId {
    let k = v as u32;
    CellId::new(Face::from_index((k / 65_536) as u8).unwrap(), 0, k % 256, (k % 65_536) / 256).unwrap()
}

fn ring() -> impl Iterator<Item = (i32, i32)> {
    let n = TILE_SIZE as i32;
    (-1..=n).flat_map(move |lj| (-1..=n).map(move |li| (li, lj))).filter(move |(li, lj)| {
        let outside_i = *li < 0 || *li >= n;
        let outside_j = *lj < 0 || *lj >= n;
        outside_i != outside_j // edge ring, excluding the 4 corners
    })
}

#[test]
fn interior_is_the_tile_itself() {
    let s = indexed_store();
    let t = TileId::new(Face::PosY, 0, 0, 0).unwrap();
    let a = gather_with_apron(&s, t).unwrap();
    assert_eq!(a.len(), (APRON_EDGE * APRON_EDGE) as usize);
    assert_eq!(a[apron_index(12, 34)], s.get(t).unwrap().get(12, 34));
}

#[test]
fn apron_crosses_faces() {
    let s = indexed_store();
    let t = TileId::new(Face::PosX, 0, 0, 0).unwrap();
    let a = gather_with_apron(&s, t).unwrap();
    // Past the +u edge of PosX lies PosY.
    assert_eq!(decode(a[apron_index(TILE_SIZE as i32, 100)]).face, Face::PosY);
}

#[test]
fn seams_are_mutual_on_every_face() {
    let s = indexed_store();
    for t in TileId::all(0) {
        let a = gather_with_apron(&s, t).unwrap();
        let last = TILE_SIZE as i32 - 1;
        for (li, lj) in ring() {
            let own = t.cell(li.clamp(0, last) as u32, lj.clamp(0, last) as u32);
            let neighbour = decode(a[apron_index(li, lj)]);
            assert!(neighbour.neighbors4().contains(&own), "{t:?} ({li},{lj}): {neighbour:?} does not see {own:?}");
        }
    }
}

#[test]
fn missing_neighbour_is_reported() {
    let mut s = LayerStore::<f32>::default();
    let t = TileId::new(Face::NegZ, 0, 0, 0).unwrap();
    s.insert(Tile::new(t));
    assert!(matches!(gather_with_apron(&s, t), Err(ApronError::MissingTile(_))));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p wb-world --test apron`
Expected: FAIL — unresolved imports `apron_index`, `gather_with_apron`, …

- [ ] **Step 3: Implement**

`crates/wb-world/src/apron.rs`:
```rust
use crate::tile::CellValue;
use crate::world::LayerStore;
use core::fmt;
use wb_grid::{TileId, TILE_SIZE};

pub const APRON_EDGE: u32 = TILE_SIZE + 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApronError {
    MissingTile(TileId),
}

impl fmt::Display for ApronError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApronError::MissingTile(t) => write!(f, "apron needs missing tile {t:?}"),
        }
    }
}

impl std::error::Error for ApronError {}

/// Index into a gathered apron buffer; `li, lj` range over `-1..=TILE_SIZE`.
pub fn apron_index(li: i32, lj: i32) -> usize {
    ((lj + 1) as u32 * APRON_EDGE + (li + 1) as u32) as usize
}

/// The tile's cells plus a one-cell ring copied from neighbouring tiles, so
/// stencil operations see identical values on both sides of every seam.
pub fn gather_with_apron<T: CellValue>(store: &LayerStore<T>, tile: TileId) -> Result<Vec<T>, ApronError> {
    let centre = store.get(tile).ok_or(ApronError::MissingTile(tile))?;
    let mut out = vec![T::default(); (APRON_EDGE * APRON_EDGE) as usize];
    let last = TILE_SIZE as i32 - 1;
    for lj in -1..=TILE_SIZE as i32 {
        for li in -1..=TILE_SIZE as i32 {
            let (ci, cj) = (li.clamp(0, last), lj.clamp(0, last));
            let value = if ci == li && cj == lj {
                centre.get(ci as u32, cj as u32)
            } else {
                let cell = tile.cell(ci as u32, cj as u32).offset(li - ci, lj - cj);
                let (nt, ni, nj) = cell.tile();
                store.get(nt).ok_or(ApronError::MissingTile(nt))?.get(ni, nj)
            };
            out[apron_index(li, lj)] = value;
        }
    }
    Ok(out)
}
```

Add to `crates/wb-world/src/lib.rs`:
```rust
mod apron;
pub use apron::{apron_index, gather_with_apron, ApronError, APRON_EDGE};
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p wb-world --test apron`
Expected: PASS (4 tests).

- [ ] **Step 5: Align the spec wording**

In the spec §3.3, replace item 3 with:
```markdown
3. **Seamless tiles** — stages read tiles through a one-cell **apron** copied from neighbouring tiles (including across cube faces), and neighbour relations are mutual on every seam; ridges and valleys continue.
```

- [ ] **Step 6: Gate and commit**

Run: `scripts/check.sh` — expected all green.
```bash
git add crates/wb-world docs/superpowers/specs
git commit -m "feat(world): seam-safe tile aprons across cube faces" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 12: Spherical geometry and vector features

**Files:**
- Create: `crates/wb-world/src/geometry.rs`, `crates/wb-world/src/feature.rs`
- Modify: `crates/wb-world/src/world.rs` (add `features` field), `crates/wb-world/src/lib.rs`
- Test: `crates/wb-world/tests/geometry.rs`

**Interfaces:**
- Consumes: `Planet`, `World` (Task 9); `LatLon`, `Vec3` (wb-grid).
- Produces:
  - `pub fn central_angle(a: LatLon, b: LatLon) -> f64`
  - `pub fn distance_m(planet: &Planet, a: LatLon, b: LatLon) -> f64`
  - `pub fn polyline_length_m(planet: &Planet, pts: &[LatLon]) -> f64`
  - `pub fn polygon_area_m2(planet: &Planet, ring: &[LatLon]) -> f64` — open ring (first ≠ last), ≥ 3 points, simple polygon smaller than a hemisphere; returns 0 for fewer than 3 points.
  - `pub struct FeatureId(pub u64)`; `pub enum Geometry { Point(LatLon), LineString(Vec<LatLon>), Polygon(Vec<LatLon>) }`; `pub struct Feature { pub id: FeatureId, pub kind: String, pub geometry: Geometry }`.
  - `pub struct FeatureSet` (`Default`) — `insert(&mut self, Feature) -> Option<Feature>`, `get(&self, FeatureId) -> Option<&Feature>`, `remove(&mut self, FeatureId) -> Option<Feature>`, `iter(&self) -> impl Iterator<Item = &Feature>` (sorted by id), `len`, `is_empty`.
  - `World` gains `pub features: FeatureSet`, initialised empty by `World::new`.

- [ ] **Step 1: Write the failing tests**

`crates/wb-world/tests/geometry.rs`:
```rust
use core::f64::consts::PI;
use wb_grid::LatLon;
use wb_world::{
    distance_m, polygon_area_m2, polyline_length_m, Extent, Feature, FeatureId, Geometry, Planet, World,
};

fn d(lat: f64, lon: f64) -> LatLon {
    LatLon::from_degrees(lat, lon)
}

#[test]
fn quarter_equator_distance() {
    let p = Planet::default();
    let got = distance_m(&p, d(0.0, 0.0), d(0.0, 90.0));
    assert!((got - PI * p.radius_m / 2.0).abs() < 1e-6);
    assert_eq!(distance_m(&p, d(10.0, 10.0), d(10.0, 10.0)), 0.0);
}

#[test]
fn polyline_length_adds_segments() {
    let p = Planet::default();
    let len = polyline_length_m(&p, &[d(0.0, 0.0), d(0.0, 45.0), d(0.0, 90.0)]);
    assert!((len - PI * p.radius_m / 2.0).abs() < 1e-6);
}

#[test]
fn octant_area_is_one_eighth_of_the_sphere() {
    let p = Planet::default();
    let area = polygon_area_m2(&p, &[d(0.0, 0.0), d(0.0, 90.0), d(90.0, 0.0)]);
    let expected = 4.0 * PI * p.radius_m * p.radius_m / 8.0;
    assert!((area - expected).abs() < 1e-9 * expected, "{area} vs {expected}");
    // Orientation does not matter.
    let reversed = polygon_area_m2(&p, &[d(90.0, 0.0), d(0.0, 90.0), d(0.0, 0.0)]);
    assert!((reversed - expected).abs() < 1e-9 * expected);
    assert_eq!(polygon_area_m2(&p, &[d(0.0, 0.0), d(1.0, 1.0)]), 0.0);
}

#[test]
fn features_live_on_the_world_in_id_order() {
    let mut w = World::new(Planet::default(), Extent::Planet);
    assert!(w.features.is_empty());
    for id in [5u64, 1, 3] {
        w.features.insert(Feature {
            id: FeatureId(id),
            kind: "mountain_range".into(),
            geometry: Geometry::LineString(vec![d(0.0, 0.0), d(1.0, 1.0)]),
        });
    }
    let ids: Vec<u64> = w.features.iter().map(|f| f.id.0).collect();
    assert_eq!(ids, vec![1, 3, 5]);
    assert_eq!(w.features.remove(FeatureId(3)).unwrap().kind, "mountain_range");
    assert_eq!(w.features.len(), 2);
    assert!(w.features.get(FeatureId(3)).is_none());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p wb-world --test geometry`
Expected: FAIL — unresolved imports `distance_m`, `Feature`, …

- [ ] **Step 3: Implement geometry**

`crates/wb-world/src/geometry.rs`:
```rust
use crate::world::Planet;
use wb_grid::{LatLon, Vec3};

/// Angle between two points as seen from the planet's centre, in radians.
pub fn central_angle(a: LatLon, b: LatLon) -> f64 {
    let (va, vb) = (a.to_vec3(), b.to_vec3());
    libm::atan2(va.cross(vb).length(), va.dot(vb))
}

pub fn distance_m(planet: &Planet, a: LatLon, b: LatLon) -> f64 {
    central_angle(a, b) * planet.radius_m
}

pub fn polyline_length_m(planet: &Planet, pts: &[LatLon]) -> f64 {
    pts.windows(2).map(|w| distance_m(planet, w[0], w[1])).sum()
}

/// Area of a simple spherical polygon (open ring, smaller than a hemisphere).
pub fn polygon_area_m2(planet: &Planet, ring: &[LatLon]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let v: Vec<Vec3> = ring.iter().map(|p| p.to_vec3()).collect();
    let a = v[0];
    let mut excess = 0.0;
    for w in v[1..].windows(2) {
        let (b, c) = (w[0], w[1]);
        let num = a.dot(b.cross(c));
        let den = 1.0 + a.dot(b) + b.dot(c) + c.dot(a);
        excess += 2.0 * libm::atan2(num, den);
    }
    excess.abs() * planet.radius_m * planet.radius_m
}
```

- [ ] **Step 4: Implement features**

`crates/wb-world/src/feature.rs`:
```rust
use std::collections::BTreeMap;
use wb_grid::LatLon;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeatureId(pub u64);

/// Resolution-independent geometry in latitude/longitude.
#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    Point(LatLon),
    LineString(Vec<LatLon>),
    /// Open ring: the first point is not repeated at the end.
    Polygon(Vec<LatLon>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    pub id: FeatureId,
    pub kind: String,
    pub geometry: Geometry,
}

#[derive(Clone, Debug, Default)]
pub struct FeatureSet {
    features: BTreeMap<FeatureId, Feature>,
}

impl FeatureSet {
    pub fn insert(&mut self, f: Feature) -> Option<Feature> {
        self.features.insert(f.id, f)
    }

    pub fn get(&self, id: FeatureId) -> Option<&Feature> {
        self.features.get(&id)
    }

    pub fn remove(&mut self, id: FeatureId) -> Option<Feature> {
        self.features.remove(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Feature> {
        self.features.values()
    }

    pub fn len(&self) -> usize {
        self.features.len()
    }

    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }
}
```

- [ ] **Step 5: Add features to the world**

In `crates/wb-world/src/world.rs`:
- Add `use crate::feature::FeatureSet;` to the imports.
- In `pub struct World`, after `pub extent: Extent,` add:
```rust
    pub features: FeatureSet,
```
- In `World::new`, after `extent,` add:
```rust
            features: FeatureSet::default(),
```

Add to `crates/wb-world/src/lib.rs`:
```rust
mod feature;
mod geometry;
pub use feature::{Feature, FeatureId, FeatureSet, Geometry};
pub use geometry::{central_angle, distance_m, polygon_area_m2, polyline_length_m};
```

- [ ] **Step 6: Run to verify pass**

Run: `cargo test -p wb-world --test geometry`
Expected: PASS (4 tests).

- [ ] **Step 7: Gate and commit**

Run: `scripts/check.sh` — expected all green.
```bash
git add crates/wb-world
git commit -m "feat(world): spherical distance/area and vector features" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 13: Cache keys, world hash, golden determinism test, CI

**Files:**
- Create: `crates/wb-world/src/hash.rs`, `.github/workflows/ci.yml`
- Modify: `crates/wb-world/src/lib.rs`
- Test: `crates/wb-world/tests/golden.rs`

**Interfaces:**
- Consumes: everything above.
- Produces:
  - `pub struct SourceHash(pub [u8; 32])` — the edit-log state digest (supplied by subsystem [2]).
  - `pub fn tile_cache_key(source: &SourceHash, seed: u64, engine_version: &str, layer_name: &str, tile: TileId) -> [u8; 32]`.
  - `pub fn world_hash(world: &World) -> [u8; 32]` — covers planet, extent, registry (in id order), every tile's content hash (in tile order), and every feature (in id order).
  - `pub fn to_hex(h: &[u8; 32]) -> String`.
  - CI running the golden test on x86-64 Linux, ARM64 Linux, ARM64 macOS, and wasm32-wasip1.

- [ ] **Step 1: Write the failing tests**

`crates/wb-world/tests/golden.rs`:
```rust
//! Cross-target determinism gate. The same world must hash identically on
//! wasm32-wasip1, x86-64, and ARM64. If this fails after an intentional
//! change to generation or encoding, update GOLDEN in the same commit.

use wb_grid::{CellId, Face, LatLon, TileId, Vec3};
use wb_world::{
    refine_tile, tile_cache_key, to_hex, world_hash, Dtype, Extent, Feature, FeatureId, Geometry, LayerDesc,
    LodPolicy, Planet, SourceHash, Tile, World,
};

const GOLDEN: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn pattern(p: Vec3) -> f32 {
    (1000.0 * libm::sin(3.0 * p.x) * libm::cos(2.0 * p.y) + 500.0 * libm::sin(5.0 * p.z)) as f32
}

fn detail(c: CellId) -> f32 {
    let p = c.center();
    (40.0 * libm::sin(40.0 * p.x + 7.0 * p.y) * libm::cos(33.0 * p.z)) as f32
}

fn build() -> World {
    let mut w = World::new(Planet::default(), Extent::Planet);
    let elev = w
        .register_layer(LayerDesc {
            name: "elevation".into(),
            dtype: Dtype::F32,
            unit: "m".into(),
            producer: "golden".into(),
            lod: LodPolicy::AreaMean,
        })
        .unwrap();
    let biome = w
        .register_layer(LayerDesc {
            name: "biome".into(),
            dtype: Dtype::U8,
            unit: "class".into(),
            producer: "golden".into(),
            lod: LodPolicy::Mode,
        })
        .unwrap();
    for t in TileId::all(0) {
        let e = Tile::from_fn(t, |i, j| pattern(t.cell(i, j).center()));
        let b = Tile::from_fn(t, |i, j| ((e.get(i, j) + 2000.0) / 500.0) as u8);
        w.layer_mut::<f32>(elev).unwrap().insert(e);
        w.layer_mut::<u8>(biome).unwrap().insert(b);
    }
    let parent = TileId::new(Face::PosZ, 0, 0, 0).unwrap();
    for c in parent.children().unwrap() {
        let tile = refine_tile(w.layer::<f32>(elev).unwrap(), c, detail).unwrap();
        w.layer_mut::<f32>(elev).unwrap().insert(tile);
    }
    w.features.insert(Feature {
        id: FeatureId(1),
        kind: "region".into(),
        geometry: Geometry::Polygon(vec![
            LatLon::from_degrees(10.0, 10.0),
            LatLon::from_degrees(10.0, 20.0),
            LatLon::from_degrees(20.0, 15.0),
        ]),
    });
    w
}

#[test]
fn golden_world_hash() {
    let got = to_hex(&world_hash(&build()));
    assert_eq!(got, GOLDEN, "world hash changed; if intentional, set GOLDEN = \"{got}\"");
}

#[test]
fn world_hash_is_sensitive_to_content() {
    let a = build();
    let mut b = build();
    let id = b.registry().id_of("elevation").unwrap();
    let t = TileId::new(Face::NegX, 0, 0, 0).unwrap();
    let store = b.layer_mut::<f32>(id).unwrap();
    let v = store.get(t).unwrap().get(0, 0);
    store.get_mut(t).unwrap().set(0, 0, v + 1.0);
    assert_ne!(world_hash(&a), world_hash(&b));
}

#[test]
fn cache_keys_separate_every_input() {
    let src = SourceHash([7; 32]);
    let t = TileId::new(Face::PosY, 3, 1, 2).unwrap();
    let base = tile_cache_key(&src, 42, "0.1.0", "elevation", t);
    assert_eq!(base, tile_cache_key(&src, 42, "0.1.0", "elevation", t));
    assert_ne!(base, tile_cache_key(&SourceHash([8; 32]), 42, "0.1.0", "elevation", t));
    assert_ne!(base, tile_cache_key(&src, 43, "0.1.0", "elevation", t));
    assert_ne!(base, tile_cache_key(&src, 42, "0.1.1", "elevation", t));
    assert_ne!(base, tile_cache_key(&src, 42, "0.1.0", "biome", t));
    assert_ne!(base, tile_cache_key(&src, 42, "0.1.0", "elevation", TileId::new(Face::PosY, 3, 2, 1).unwrap()));
    // Length-prefixing prevents "ab"+"c" colliding with "a"+"bc".
    assert_ne!(tile_cache_key(&src, 1, "ab", "c", t), tile_cache_key(&src, 1, "a", "bc", t));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p wb-world --test golden`
Expected: FAIL — unresolved imports `tile_cache_key`, `to_hex`, `world_hash`, `SourceHash`.

- [ ] **Step 3: Implement hashing**

`crates/wb-world/src/hash.rs`:
```rust
use crate::extent::Extent;
use crate::feature::Geometry;
use crate::tile::{CellValue, Dtype};
use crate::world::{LayerStore, World};
use wb_grid::{LatLon, TileId};

/// Digest of the edit-log state a world was generated from (subsystem [2]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceHash(pub [u8; 32]);

fn put_str(h: &mut blake3::Hasher, s: &str) {
    h.update(&(s.len() as u64).to_le_bytes());
    h.update(s.as_bytes());
}

fn put_tile_id(h: &mut blake3::Hasher, t: TileId) {
    h.update(&[t.face.index(), t.level]);
    h.update(&t.x.to_le_bytes());
    h.update(&t.y.to_le_bytes());
}

fn put_f64(h: &mut blake3::Hasher, v: f64) {
    h.update(&v.to_bits().to_le_bytes());
}

fn put_points(h: &mut blake3::Hasher, pts: &[LatLon]) {
    h.update(&(pts.len() as u64).to_le_bytes());
    for p in pts {
        put_f64(h, p.lat);
        put_f64(h, p.lon);
    }
}

/// Content address of one generated tile.
pub fn tile_cache_key(source: &SourceHash, seed: u64, engine_version: &str, layer_name: &str, tile: TileId) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(b"wb-tile-key-v1");
    h.update(&source.0);
    h.update(&seed.to_le_bytes());
    put_str(&mut h, engine_version);
    put_str(&mut h, layer_name);
    put_tile_id(&mut h, tile);
    h.finalize().into()
}

fn put_store<T: CellValue>(h: &mut blake3::Hasher, store: &LayerStore<T>) {
    h.update(&(store.len() as u64).to_le_bytes());
    for (id, tile) in store.iter() {
        put_tile_id(h, *id);
        h.update(&tile.content_hash());
    }
}

/// Digest of an entire world, independent of platform.
pub fn world_hash(world: &World) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(b"wb-world-v1");
    put_f64(&mut h, world.planet.radius_m);
    match world.extent {
        Extent::Planet => {
            h.update(&[0]);
        }
        Extent::Region(r) => {
            h.update(&[1]);
            for v in [r.south, r.north, r.west, r.east] {
                put_f64(&mut h, v);
            }
        }
    }
    for (id, d) in world.registry().iter() {
        put_str(&mut h, &d.name);
        h.update(&[d.dtype as u8, d.lod as u8]);
        put_str(&mut h, &d.unit);
        put_str(&mut h, &d.producer);
        match d.dtype {
            Dtype::F32 => put_store(&mut h, world.layer::<f32>(id).expect("registered")),
            Dtype::U8 => put_store(&mut h, world.layer::<u8>(id).expect("registered")),
            Dtype::U16 => put_store(&mut h, world.layer::<u16>(id).expect("registered")),
            Dtype::I16 => put_store(&mut h, world.layer::<i16>(id).expect("registered")),
        }
    }
    h.update(&(world.features.len() as u64).to_le_bytes());
    for f in world.features.iter() {
        h.update(&f.id.0.to_le_bytes());
        put_str(&mut h, &f.kind);
        match &f.geometry {
            Geometry::Point(p) => {
                h.update(&[0]);
                put_points(&mut h, core::slice::from_ref(p));
            }
            Geometry::LineString(pts) => {
                h.update(&[1]);
                put_points(&mut h, pts);
            }
            Geometry::Polygon(pts) => {
                h.update(&[2]);
                put_points(&mut h, pts);
            }
        }
    }
    h.finalize().into()
}

pub fn to_hex(h: &[u8; 32]) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}
```

Add to `crates/wb-world/src/lib.rs`:
```rust
mod hash;
pub use hash::{tile_cache_key, to_hex, world_hash, SourceHash};
```

- [ ] **Step 4: Run and capture the golden hash (native)**

Run: `cargo test -p wb-world --test golden`
Expected: `cache_keys_separate_every_input` and `world_hash_is_sensitive_to_content` PASS; `golden_world_hash` FAILS with the message `world hash changed; if intentional, set GOLDEN = "<64 hex chars>"`.
Copy those 64 hex characters into the `GOLDEN` constant in `crates/wb-world/tests/golden.rs`.

- [ ] **Step 5: Verify the same hash on every local target**

Run: `cargo test -p wb-world --test golden`
Expected: PASS (3 tests).
Run: `cargo test -p wb-world --test golden --target wasm32-wasip1`
Expected: PASS (3 tests) — WASM produces the identical hash.
Run: `cargo test --release -p wb-world --test golden`
Expected: PASS — optimisation level does not change results.

- [ ] **Step 6: Add CI**

`.github/workflows/ci.yml`:
```yaml
name: ci
on:
  push:
    branches: [main]
  pull_request:

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: rustup show
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets -- -D warnings

  test:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, ubuntu-24.04-arm, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - run: rustup show
      - run: cargo test --workspace

  wasm:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: bytecodealliance/actions/wasmtime/setup@v1
      - run: rustup show
      - run: cargo test --workspace --target wasm32-wasip1
```

- [ ] **Step 7: Gate, commit, push, and confirm CI**

Run: `scripts/check.sh` — expected all green.
```bash
git add crates/wb-world .github
git commit -m "feat(world): content-addressed hashing, golden cross-target test, CI" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
git push
gh run watch --exit-status
```
Expected: all jobs green — `lint`, `test` on ubuntu-latest (x86-64), ubuntu-24.04-arm (ARM64), macos-latest (ARM64), and `wasm`. A `golden_world_hash` failure on any single target means that target is not bit-exact: investigate before continuing; never special-case the golden per target.

---

## Spec coverage (self-review)

| Spec requirement | Task |
|---|---|
| §2.3.1 Determinism (pure-Rust math, no FMA, fixed-order, CI golden on WASM/x86-64/ARM64) | 1, 13 |
| §2.3.4 Engine version recorded | 9 (`ENGINE_VERSION`), 13 (cache key) |
| §2.3.5 Derived data disposable, cache-keyed by hash | 7 (encode/decode), 13 (`tile_cache_key`) |
| §3.1 Equi-angular cube-sphere, 256×256 quadtree tiles, `(face, level, x, y)` | 3, 4 |
| §3.1 Level table (0 … 7+) | 4 (`MAX_LEVEL = 12`) |
| §3.2 Layer registry (name, dtype, unit, producer, LOD policy) | 9 |
| §3.2 Vector features in lat/lon | 12 |
| §3.3.1 Coarse is truth | 10 |
| §3.3.3 Seamless tiles | 5, 11 |
| §3.4 Whole-planet and regional extents | 8 |
| §3.5 Content-addressed tile cache key | 13 |
| §7D lore-fact checks need areas/distances (e.g. "≈ 600,000 sq mi") | 12 |
| §3.3.2 Water downhill, §3.3.4 detail follows landscape, level-0 regional context | Out of scope (Hydrology, Terrain, Pipeline specs) |
