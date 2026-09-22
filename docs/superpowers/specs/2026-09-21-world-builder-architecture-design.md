# World_Builder — Product Architecture Design

- **Date:** 2026-09-21
- **Status:** Approved in brainstorming; revised 2026-09-21 after Aethoria vault review; awaiting written-spec review
- **Scope:** Whole-product architecture. Each subsystem gets its own spec → plan → implementation cycle; this document fixes the boundaries, invariants, and interfaces between them.
- **Diagram:** [`docs/diagrams/system-map.html`](../../diagrams/system-map.html) (source: `system-map.architecture.json`)

---

## 1. Product

A commercial, web-first fantasy world-building engine. Customers import or sketch a world; the engine generates scientifically plausible terrain, climate, hydrology, biomes, and civilizations **around the customer's own requirements**, reports how plausible each requirement is, and renders the result as globes, maps, atlas art, print files, and data exports.

### 1.1 Differentiators (vs Inkarnate, Azgaar, Wonderdraft)

1. **Plausibility feedback** — every user requirement gets a verdict, mechanism, conflicts, consequences, and one-click fixes; the user may keep anything anyway.
2. **Auto-drawing** — rough strokes become geology, and the art follows from the geology, so non-artists with no geographic knowledge get professional, believable maps.
3. **One world, many outputs** — a single world model drives globe, flat projections, styled art, print, and game-engine/tool exports.
4. **Civilization in time** — user-placed settlements and nations get a plausible generated history and seeded, branchable projected futures.
5. **Lore-aware** — written world facts (areas, climates, exports, populations, astronomy) are checked like drawn features, map entities link to lore notes, and the engine writes gazetteer entries back (Obsidian-compatible).
6. **Magic as a first-class system** — user-defined fields such as oryn/mana, with sources, flow, and seasonal cycles, shape climate, biomes, resources, and creatures just as physics does.

### 1.2 Audiences

All four are supported through **"Start as…" presets** that change defaults (style, grid overlay, exports, amount of UI shown), never the engine:

| Preset | Emphasis |
|---|---|
| Game Master | Regional maps, hex/square overlays, settlements & roads, printable handouts |
| Author | Atlas-style maps, print resolution, travel-time consistency, histories |
| Game Developer | Heightmaps, masks, Unity/Unreal/Godot exports |
| World-Builder | Whole planet, full simulation depth, all realism controls |

### 1.3 Business decisions

- **Delivery:** Web SaaS first. A desktop (Tauri) one-time-purchase edition remains possible later because ~80–90% of code (engine, renderer, editor, I/O) is shell-independent.
- **Pricing:** Undecided until real per-job compute costs are known. The system is **designed for subscription + credits**: every expensive operation is metered, plans are a pluggable entitlements layer, and credits live in an append-only ledger.
- **Collaboration:** Ship **solo editing + share links (view/comment) + public gallery (publish/remix)** first. The data model is **CRDT-ready** so real-time co-editing can be added without a rewrite.
- **Narrative:** The engine emits structured history/event data. An **optional AI chronicle** (prose written from simulated events, never inventing facts) is a later paid add-on.

---

## 2. Architecture overview

### 2.1 Subsystems

| # | Subsystem | Tier | Responsibility |
|---|---|---|---|
| 1 | World Model & Spherical Grid | Engine | Planet representation, grid layers, vector features, LOD, caching keys |
| 2 | Edit Log | Engine | Append-only, CRDT-ready log of every user edit; the only durable source of truth |
| 3 | Constraint & Feasibility | Engine | Turns edits into constraints; aggregates verdicts; realism dial; overrides |
| 4 | Physical Pipeline | Engine | Tectonics → elevation/erosion ⇄ climate → hydrology → biomes |
| 5 | Auto-Draw | Engine | Sketch strokes → feature constraints; suggestion ghosts |
| 6 | Civilization & Time | Engine | Human layers, retrodicted history, projected futures, branches, calendar |
| 7 | Importers | Engine | External formats → base constraints (coastline mask, features) |
| 8 | Exporters | Engine | World → heightmaps, masks, vectors, tool formats, print |
| 9 | Services | Backend | Accounts, projects, sharing, gallery, job queue, workers, metering, entitlements, credit ledger |
| 10 | Editor UI | Front end | Drawing tools, planet panel, inspector, feasibility report, timeline, onboarding |
| 11 | Renderer | Engine (Rust/wgpu) | Globe + flat projections, style sheets, labels; same code on screen and for print |
| 12 | AI Chronicle | Backend (later) | Optional prose from structured events |
| 13 | Astronomy & Calendar | Engine | Stars (incl. multiple suns), moons, tides, orbital feasibility, calendar builder, night sky data |
| 14 | Custom Fields & Rules | Engine | User-defined fields (e.g., oryn) with sources, flow, seasonality, and effect rules on other layers |
| 15 | Ecology & Species | Engine | Species/race definitions, habitat preferences, range maps, population plausibility |
| 16 | Codex & Lore | Engine + Backend | Entity ↔ lore-note links, written-fact constraints, gazetteer generation, Obsidian import/export |

### 2.2 Technology

- **Engine core:** Rust, compiled to **WASM** (browser, runs in a Web Worker) and **native** (cloud workers). One codebase.
- **Renderer:** Rust + **wgpu** — WebGPU in browser with WebGL2 fallback; headless native on workers. Guarantees print output matches the screen.
- **Front end:** TypeScript + React + Vite; Zustand for UI state. World state lives in the engine.
- **API:** Rust (axum), sharing engine types.
- **Data:** Postgres (managed, e.g., Neon); S3-compatible object storage (Cloudflare R2, zero egress).
- **Queue:** Postgres job table (`FOR UPDATE SKIP LOCKED`) initially; dedicated broker only when load justifies.
- **Auth:** managed provider (Clerk or WorkOS). **Billing:** Stripe with usage meters.
- **Hosting:** cloud-neutral containers. Front end on Vercel or Cloudflare Pages; API + workers on Fly.io or AWS containers; GPU optional (software rasterizer fallback).

### 2.3 Cross-cutting invariants

1. **Determinism.** `(edit log, seed, engine version)` → bit-identical world on WASM, native x86-64, and native ARM64. Enforced by: pure-Rust math (no platform libm), no FMA contraction, no reassociation, fixed-order reductions (no order-dependent parallel sums). CI golden-hash tests across all three targets.
2. **One-way flow.** User edits → edit log → engine → world layers → renderer. The engine never mutates user edits; it reads and reports on them.
3. **Uniform stage contract.** Every engine stage: `(grid, relevant constraints, params, LOD, field hooks) → (layers, verdicts, events)`. Field hooks let Custom Fields & Rules (§7A) modify a stage's inputs/outputs without changing its code. Stages are independently testable and replaceable.
4. **Pinned engine versions.** Every saved world records the engine version that produced it. Upgrades never silently change a world; the user opts in to regenerate.
5. **Derived data is disposable.** Only the edit log (plus project metadata) must be durable. All tiles are regenerable and cache-keyed by hash.

### 2.4 Build order (bottom-up)

1 World Model → 2 Edit Log → 3 Constraints → 13 Astronomy & Calendar → 4 Pipeline (one stage at a time; the stage contract includes Custom-Fields hooks from the start) → 14 Custom Fields & Rules → 7 Importers (PNG, Azgaar) → 11 Renderer → 10 Editor → 9 Services → 5 Auto-Draw → 15 Ecology & Species → 8 Exporters → 16 Codex & Lore → 6 Civilization & Time → 12 AI Chronicle.

---

## 3. World Model & Spherical Grid [1]

### 3.1 Grid

- **Equi-angular cube-sphere**: 6 cube faces projected to the sphere with an equi-angular mapping (cell-area ratio ≈ 1.4 worst case).
- Each face is a **quadtree of 256×256-cell tiles**; each level splits a tile into 4.
- Tile ID: `(face, level, x, y)`.

| Level | Cell size (Earth radius) | Purpose |
|---|---|---|
| 0 | ~39 km | Plates, global climate, regional-project context |
| 2–3 | ~10–5 km | Interactive browser preview |
| 5 | ~1.2 km | Detailed erosion, regional maps |
| 7+ | ≤ ~300 m | Print renders on cloud workers, on demand |

Rejected alternatives: equirectangular raster (pole distortion corrupts simulation), icosahedral/H3 hex grids (non-rectangular; awkward for GPU textures, exports, and tiled jobs).

### 3.2 Contents

- **Layer registry** — typed per-cell fields, each with name, dtype, unit, producing stage, and LOD policy. Initial set: `elevation` (m), `plate_id`, `boundary_type`, `crust_type`, `crust_age`, `uplift`, `rock_hardness`, `sediment`, `water_flux`, `temperature[season]`, `precipitation[season]`, `wind[season]`, `koppen`, `biome`, `vegetation_density`, `carrying_capacity`, `travel_cost[mode]`, `resource_*`.
- **Vector features** — rivers (with flow size), coastlines, roads, borders, settlements, labels — stored in lat/lon on the sphere, resolution-independent.
- **User edits are stored in lat/lon**, so one drawing drives preview and print identically.

### 3.3 Level-of-detail guarantees (enforced by property tests)

1. **Coarse is truth** — each 2×2 child block's **area-weighted** mean elevation equals its parent cell's elevation, to f32 rounding (≤ 2 mm for elevations). Refinement adds detail; it never moves features.
2. **Water runs downhill at every level** — drainage is solved coarse and refined; every river cell drains to a lower neighbour; lakes neither appear nor vanish across zoom.
3. **Seamless tiles** — stages read tiles through a one-cell **apron** copied from neighbouring tiles (including across cube faces), and neighbour relations are mutual on every seam; ridges and valleys continue.
4. **Detail follows the landscape** — refinement detail is conditioned on the terrain's own properties (young sharp ridges, old rounded ranges, flat floodplains), not uniform noise.

### 3.4 Extent

- **Whole planet** — all six faces; wraps.
- **Regional** — a lat/lon extent on the sphere plus **context**: user-described surroundings (e.g., open ocean west, land east). Only intersecting tiles are simulated in detail; the rest of the planet is synthesized cheaply at level 0 from the context so winds, currents, and plates enter the region plausibly.

### 3.5 Caching

Tile cache key = `hash(edit-log state, seed, engine version, level, tile id)`. Caches are content-addressed, dedupe automatically, and are safe to evict.

---

## 4. Edit Log [2]

- Append-only sequence of typed operations (add/modify/delete constraint, set planet param, place entity, add intervention, accept suggestion, override verdict, import).
- Every operation has a stable ID, author, logical timestamp (Lamport/HLC), and causal parents — **CRDT-ready**: designed so concurrent logs merge deterministically. v1 has a single writer per world.
- Undo/redo are operations over the log.
- Snapshots (compacted state at a log position) accelerate loading; gallery publishes are immutable snapshots.
- The log plus seed plus engine version fully determine the world.

---

## 5. Constraint & Feasibility System [3]

### 5.1 Constraints

- **Features:** mountain range, hills, canyon, river (path + direction), lake, desert, forest, glacier, volcano, island chain, coastline — geometry plus optional attributes (target peak height, age, flow direction, etc.).
- **Region rules (fantasy overrides):** painted areas with a rule, e.g. "permanently frozen", "floating isles", "perpetual storm". Simple rules are presets built on Custom Fields & Rules (§7A).
- **Lore facts:** written statements attached to entities — e.g. "area ≈ 600,000 sq mi", "bounded north by the Thulean Mountains", "temperate maritime with frequent mists", "exports wine from southern valleys". Each is a typed, checkable assertion (§7D).
- **Authored events:** dated cataclysms and eras on the geological and historical timelines, e.g. "inland sea drains when its eastern barrier collapses, ~50,000 years ago" (§7C, §7.4).
- **Authored plates (optional):** a user-supplied plate layout with motion vectors, used as hard or target input instead of being searched.
- **Civilization placements:** settlements, nations, borders, routes (§7).
- **Strength:** `hard` (must hold exactly, e.g. imported coastline, overridden features) or `target` (aim for, e.g. "peaks ≈ 5 km").

### 5.2 Solving

Each stage runs its forward model **plus a bounded search** over its free variables (e.g., candidate plate configurations) scored against relevant constraints. Search budgets scale with context: small in browser (interactive), larger on cloud workers.

### 5.3 Verdicts

Per constraint:

- **Grade:** ✅ Plausible · ⚠️ Stretch · ❌ Implausible, with a 0–1 score.
- **Mechanism:** why it works (e.g., "ancient collision belt, 300 Myr, eroded").
- **Conflicts:** with physics or other constraints.
- **Consequences:** downstream effects (e.g., rain shadow → steppe).
- **Suggestions:** concrete one-click fixes, each an edit-log operation (undoable, deterministic).
- **Status:** open · adjusted · **kept anyway** (becomes hard, optional lore reason, shown as *intentional*).

"Keep anyway" forces the feature (e.g., injected uplift, carved channel); **all downstream stages still respond** to it.

### 5.4 Realism dial

Three **presets — Earth-strict, Plausible fantasy, High fantasy — each setting a continuous slider**, which the user can fine-tune; per-constraint tolerances optional. The dial changes grading thresholds, **not the physics**.

### 5.5 Explanations as data

Verdicts are `code + parameters`; human text is produced by localized templates. The same data feeds map pins, the report panel, and the future AI chronicle.

---

## 6. Physical Pipeline [4]

```
Tectonics → Elevation → Climate (pass 1) → Erosion → Climate (pass 2) → Hydrology → Biomes
```

Climate and erosion are coupled via two passes. Every stage applies region rules inside their painted areas and is versioned so it can be replaced with a more scientific implementation later.

### 6.1 Tectonics
- If the user supplied plates, they are used as constraints; otherwise plates are searched.
- Seeded, noise-perturbed plate growth on the sphere; continental/oceanic from the land mask; per-plate Euler-pole motion; boundary classification (continental collision, subduction, island arc, rift/ridge, transform) from relative velocity; hotspots; **ancient orogens** for old, eroded ranges.
- Search: candidate plate layouts scored against coastline and feature constraints.
- Outputs: `plate_id`, boundary type/strength, crust type/age, uplift, rock hardness.

### 6.2 Elevation & erosion
- Isostatic base relief (continents high, ocean floor low), shelves, ridges, trenches; uplift profiles per boundary type. **Gravity bounds maximum relief and slope.**
- Erosion: stream-power (Braun–Willett implicit solver) + hillslope diffusion + thermal/talus. Glacial carving in a later version. Rock hardness modulates erosion (canyons, badlands).
- The imported coastline is enforced as hard.

### 6.3 Climate (parameterized, not a full GCM)
- Insolation from **all stars** in the Astronomy module (§7B) — including seasonal contributions from a companion sun — plus axial tilt; lapse rate; continentality.
- Circulation cell count scales with rotation rate; prevailing winds; seasonal migration of the equatorial rain belt (ITCZ).
- Ocean gyres from wind, Coriolis, and continental blocking; warm/cold currents affect coasts.
- Moisture advection with orographic precipitation and rain shadows.
- Outputs: seasonal temperature, precipitation, winds, currents; Köppen classes.
- **Weather:** v1 derives typical-weather layers (storm tracks, monsoons, cyclone zones). Animated weather is later.

### 6.4 Hydrology
- Priority-flood routing that preserves basins: lakes (including endorheic salt lakes), river networks, deltas, watersheds. Rivers vectorized with discharge.
- User-drawn rivers are hard paths; uphill violations are reported, and "keep anyway" carves them.

### 6.5 Biomes
- Custom biomes (e.g. metal-infused grassland, piezo-crystal badlands, fluid-sand desert) are defined via Custom Fields & Rules and slot into the same classification.
- Köppen + Whittaker (temperature/precipitation), modified by drainage, soil, elevation, and water proximity; ice sheets and snowlines.
- Outputs: `biome`, `vegetation_density`, renderer visual classes.
- Painted biome constraints are checked against climate.

---

## 7. Civilization & Time [6]

### 7.1 Entities (user-created, become constraints)
Races and peoples (defined as species with civilization traits, §7E: lifespan, reproduction, habitat preference); cultures (subsistence, tech era, optional fantasy traits); settlements (point, name, size class, role, optional founding year); nations (name, capital, borders drawn or suggested); routes and landmarks.

### 7.2 Derived human layers
- **Carrying capacity** from climate, soil, water, fishing, herding.
- **Travel cost** per mode (foot, horse, cart, boat) and season: slope, rivers, fords, passes, biome, snow.
- **Geology-derived resources:** ore in orogenic/volcanic zones, salt in evaporite basins, timber, fertile deltas.

### 7.3 Present
Site suggestions (water, farmland, defensibility, harbours, crossings, route junctions) and verdicts for each placement. **Travel-time queries** ("Kaldros → Veyl: 19 days by horse in summer, 31 in winter, over the Iron Pass") that also check author-stated travel times.

### 7.4 Past (retrodiction)
Candidate histories are simulated forward from a seeded early state, then scored against the user's present **and against authored eras and events** (e.g., cataclysms that collapse civilization and restart the calendar); the best is kept, with minimal forcing. Agent-based at settlement/nation granularity (thousands of settlements, hundreds of nations, 1–10-year ticks). Outputs: event log, per-era map snapshots, population/territory series. Engine-invented entities are flagged and renameable; an optional culture-aware name generator is available.

### 7.5 Future (projection)
Seeded forward simulation over a chosen horizon (100/500/1000 years), with dials for conflict, tech growth, and climate/magical shocks. **Interventions** are dated edit-log operations. **Branches** = seed + dials + interventions: deterministic, cheap to store, comparable side by side.

### 7.6 Time
The calendar comes from the Astronomy & Calendar module (§7B): year and day lengths, moon cycles, custom months, weeks, and named seasons. Users name eras and epochs (e.g., "PC3"), including restarts after cataclysms. A timeline slider scrubs past eras and projected futures. Terrain is fixed on human timescales in v1.

Small scopes run in the browser; long histories and large futures run as metered cloud jobs.

---

## 7A. Custom Fields & Rules [14]

Generalizes fantasy overrides into a first-class, deterministic system:

- **Fields:** user-defined scalar/vector layers (e.g., `oryn`) registered in the layer registry, with units and ranges.
- **Sources & structures:** networks (e.g., deep ley lines with flow from high to low concentration), point sources (surface access points), transient sources (vents with lifetimes that create mineral deposits), painted zones.
- **Dynamics:** diffusion/flow along structures, decay, seasonal modulation driven by Astronomy (e.g., peak during a twin-sun season).
- **Effects:** declarative rules that modify other stages' inputs/outputs: climate (perpetual storms, magical weather), terrain (floating mountains), biomes (custom biome types), resources (novel metals, crystals), ecology (oryn-seeking species).
- **Rule language:** a small, sandboxed, declarative expression language compiled to the engine's deterministic evaluator — no arbitrary code, bounded cost, versioned.
- **Presets:** simple region rules ("permanently frozen", "no rain") ship as presets built on this system.
- **Verdicts:** rules are reported as *intentional* effects; interactions are explained (e.g., "the storm region's rainfall feeds these three rivers").

## 7B. Astronomy & Calendar [13]

- **Bodies:** one or more stars (luminosity, orbit — including distant companions), the planet's orbit, and any number of moons (mass, distance, period).
- **Derived:** insolation per season from all stars; tides from all moons (combined tidal ranges feed coastal and hydrology stages); day/year lengths; eclipses and conjunctions as events.
- **Feasibility:** orbital stability and period checks — e.g., a companion sun orbiting among the gas giants would have a multi-decade period and destabilize outer planets; the report explains this and offers workable configurations or "keep anyway".
- **Calendar builder:** custom month/week/day names, named seasons ("The Brightening", "Dual Light"), leap rules, and epochs.
- **Sky data:** positions and phases for a night-sky view in the renderer.
- **Planet parameters** remain in the Planet panel; radius and gravity are independent (a larger planet may keep Earth-like gravity via a low-density core).

## 7C. Deep time

- The geological timeline accepts **authored events** (cataclysms, sea drainage, uplift episodes) that tectonics and erosion must reproduce; the historical timeline accepts authored eras and events (§7.4). Both appear on the same timeline slider.

## 7D. Codex & Lore [16]

- **Entities have notes.** Every map entity (region, settlement, feature, species, magic site, landmark) can link to rich lore notes.
- **Lore-fact constraints.** Typed assertions (area, bounding features, climate class, terrain type, elevation/height, population, exports/imports, travel times) receive verdicts like any constraint. Example: "Astoria's southern valleys suit viticulture ✅".
- **Gazetteer generation.** For every region the engine produces structured atlas entries — terrain, climate, resources, likely exports/imports, travel times, demographics — which the user can accept, edit, or compare to their own text.
- **Obsidian integration.** Import a vault's notes as linked codex entries (frontmatter + wikilinks preserved); export map entities and gazetteer entries as markdown with frontmatter.
- **Later (optional, AI add-on):** assisted extraction of candidate lore facts from notes, each approved by the user before becoming a constraint.

## 7E. Ecology & Species [15]

- **Species definitions:** habitat preferences (climate, biome, elevation, water, field values such as oryn), diet/trophic level, body size, lifespan, reproduction.
- **Outputs:** range maps, population estimates from carrying capacity (megafauna plausibility), migration corridors.
- **Races** are species with civilization traits; their demographics feed Civilization & Time and receive verdicts (e.g., "a 700,000-person capital in a 20-million world needs this food hinterland").

---

## 8. Front end [10, 11, 5]

### 8.1 Renderer [11]
- Rust + wgpu; one renderer for screen and print.
- Views: 3D globe (relief exaggeration, atmosphere); flat projections (equirectangular, Mercator, Robinson, orthographic, conic for regions); layer switcher; era slider.
- **Declarative style sheets** map world layers to symbology. Built-in styles: **Data**, **Satellite**, **Atlas** (hand-painted look: hillshade/hachures, painted forests, parchment and ink). Future: style marketplace.
- Automatic collision-free label placement, including curved labels along ranges and rivers.
- Tiles stream by camera LOD.

### 8.2 Auto-Draw [5]
- Feature brushes with stylus pressure: mountains, hills, canyon, river, lake, forest, desert, volcano, island chain, glacier.
- **Strokes become constraints (geology), not pixels**; the art follows from the generated terrain.
- **Suggest mode:** ghost features the user can accept with one click ("a range here would explain this coastline").
- v1 is purely algorithmic; ML style models are a later option.

### 8.3 Editor [10]
- Layout: top bar (Planet panel, realism preset + slider, Start-as preset, Generate); left tools (select, brushes, places, rules, import); centre canvas; right inspector + feasibility report; bottom timeline.
- **Planet panel:** radius and gravity; axial tilt and seasons; rotation rate and direction; sea level and ocean coverage; atmosphere/greenhouse; link to the Star System editor (§7B); fantasy region rules. Earth defaults.
- **Units:** metric or imperial per project (the Aethoria lore uses miles and feet).
- **Scale overlays:** drop real-world outlines (e.g., USA, Africa) onto the map at true, projection-correct scale; shows how projections inflate polar landmasses.
- Engine runs in a Web Worker; **coarse result first, then refinement**; only affected stages and tiles recompute.
- Undo/redo via the edit log.
- Onboarding: Start-as presets, templates, guided first-mountain tutorial.
- Accessibility: colour-blind-safe palettes, full keyboard, screen-reader-readable report, stylus/tablet support.

---

## 9. Services [9]

- **Accounts & auth** via managed provider.
- **Projects:** metadata, edit logs, snapshots in Postgres; tiles and exports in R2 behind signed URLs.
- **Sharing:** view/comment links. **Gallery:** immutable published snapshots, publisher-chosen remix license, reporting and moderation.
- **Jobs:** Postgres-backed queue; containerized native workers (engine + headless renderer); GPU optional.
- **Metering & entitlements:** every expensive operation (print render, deep erosion, long history/future, AI chronicle) is metered; plans are a pluggable entitlements map; credits are recorded in an append-only ledger. Stripe integration.
- **Security:** private by default; tenant isolation; rate limits; untrusted uploads parsed in sandboxed workers.

---

## 10. Import / Export [7, 8]

**Importers**
- **v1: PNG/JPG** — automatic land/ocean colour detection with user confirmation. Whole-planet images need not be exactly 2:1 (e.g., 5798×3100): the user confirms the latitude span covered and how the image maps to the sphere; otherwise the user places a region's lat/lon extent.
- **Early: Azgaar `.map`** — imports heightmap, coastlines, rivers, and optionally states/burgs, with an option to discard Azgaar's generated names and cultures in favour of the user's own lore.
- **Intent layers:** additional images (hand-drawn climate bands, plate sketches) can be imported as optional *target* constraints, never as hard truth.
- Inkarnate/Wonderdraft exports use the image path with a smarter classifier that ignores icons and labels.
- **Later:** Azgaar `.map`, SVG, GeoJSON, shapefile, GeoTIFF heightmaps.

**Exporters**
- 16-bit PNG/TIFF heightmaps (equirectangular or cube faces); biome, climate, and precipitation masks; GeoJSON/SVG vectors; Azgaar `.map`; Unity/Unreal/Godot terrain (raw 16-bit + splatmaps); print (300-dpi TIFF/PDF; CMYK later).

---

## 11. Quality & testing

- **Property tests** (proptest) for LOD guarantees, drainage, tile seams, and edit-log merge convergence, over thousands of random worlds.
- **Cross-target determinism:** CI golden hashes on WASM, x86-64, and ARM64.
- **Flagship world — Aethoria:** the author's own world (two suns, three moons, 30-hour day, 390-day year, ley lines and oryn fields, authored cataclysms) is the reference showcase and a regression suite for fantasy features, alongside Earth.
- **Earth validation suite:** import Earth's real coastline; compare generated climate zones, rain shadows, and major basins against reality — the scientific acceptance test.
- **Performance budgets** in CI (e.g., preview-level regeneration in the browser within a few seconds; the exact budget is set in the pipeline spec).
- **Visual regression** for renderer styles; **Playwright** end-to-end tests for the editor.

---

## 12. Repository layout

```
world_builder/
  Cargo.toml                 # Rust workspace
  crates/
    wb-grid  wb-world  wb-editlog  wb-constraints
    wb-tectonics  wb-terrain  wb-climate  wb-hydro  wb-biome
    wb-civ  wb-autodraw  wb-io  wb-render
    wb-astro  wb-fields  wb-ecology  wb-codex
    wb-wasm  wb-server  wb-worker
  apps/web/                  # React + Vite editor (pnpm)
  docs/superpowers/specs/    # this and subsystem specs
  docs/superpowers/plans/
  docs/diagrams/
```

The project note lives in the IronCodex vault at `1 - Projects/World_Builder/World_Builder.md`; code lives only in this repository (`~/dev/world_builder`), outside iCloud.

---

## 13. Out of scope for this document

Detailed algorithms, data schemas, APIs, and performance budgets belong to each subsystem's own spec. The first subsystem spec is **[1] World Model & Spherical Grid**, followed by **[2] Edit Log**.

## 14. Open decisions (deferred deliberately)

- Final pricing and plan limits (after measuring per-job compute cost).
- Auth provider choice between Clerk and WorkOS; front-end host between Vercel and Cloudflare Pages; worker host between Fly.io and AWS — decided in the Services spec.
- Timing of the desktop (Tauri) edition and of real-time co-editing.
- Syntax of the Custom Fields rule language (decided in its own spec).
- Depth of Obsidian integration (one-way import vs. two-way sync).
