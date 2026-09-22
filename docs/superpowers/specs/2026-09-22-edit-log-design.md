# Edit Log — Subsystem [2] Design

- **Date:** 2026-09-22
- **Status:** Approved in brainstorming; awaiting written-spec review
- **Parent spec:** `docs/superpowers/specs/2026-09-21-world-builder-architecture-design.md` (§2.3 invariants, §4 Edit Log)
- **Depends on:** subsystem [1] (`wb-grid`, `wb-world`) — `LatLon`, `Geometry`, `SourceHash`
- **Crate:** `crates/wb-editlog`

---

## 1. Purpose

The Edit Log is the only durable truth about a world. It records every user action as an operation in a causal graph, materializes the current world state deterministically, provides undo/redo, named versions, and branches, and serializes everything into a single local-first project file (`.wbworld`). All generated data (tiles, verdicts, histories) is derived from its state and is disposable.

## 2. Decisions (from brainstorming)

| Topic | Decision |
|---|---|
| History features | Undo/redo + named versions + branches in v1. Data model supports branch **merge** later (not built now). |
| Storage | Local-first: one `.wbworld` file per world; browser keeps per-op records and exports/imports the file. Cloud sync (Services [9]) and the desktop edition reuse the same bytes later. No paid services before alpha/beta. |
| Architecture | Our own operation graph (no CRDT library). A text CRDT (e.g. Loro) may be adopted later only for rich-text lore notes in Codex [16]. |

## 3. Data model

### 3.1 Identifiers

| Type | Definition | Notes |
|---|---|---|
| `ActorId` | `[u8; 16]` | One per device/session; supplied by the caller (the log never generates randomness). |
| `AuthorId` | `[u8; 16]` | The human account/profile; display name lives in file META. |
| `OpId` | `{ lamport: u64, actor: ActorId }` | Total order: `lamport`, then `actor` bytes. |
| `EntityId` | `OpId` of the transaction that created the entity | Globally unique without coordination. |
| `EntityId::PLANET` | `OpId { lamport: 0, actor: [0; 16] }` | Reserved singleton for planet parameters. Exists implicitly: writes to it need no `create`, it cannot be deleted, and materialized state always contains it with `kind = "planet"`. |
| `FieldKey` | UTF-8 string, 1–64 bytes, `[a-z0-9_.]` | e.g. `spine`, `peak_m`, `axial_tilt_deg`. |
| `EntityKind` | UTF-8 string, 1–64 bytes, dotted namespace `[a-z0-9_.]` | e.g. `feature.mountain_range`, `civ.settlement`. Stored as the reserved field `kind`. |
| `VersionId` | `{ actor: ActorId, seq: u32 }` — `seq` counts versions saved by that actor | Versions are metadata, not ops; unique without coordination. |
| `AssetRef` | blake3 hash `[u8; 32]` of the asset bytes | Content-addressed, deduplicated. |

Lamport rule: a new op's `lamport = 1 + max(lamport of every op in the log)`. This guarantees every op sorts after all ops it could have observed.

### 3.2 Values

```
Value = Null | Bool(bool) | Int(i64) | Float(f64) | Text(String)
      | LatLon(LatLon) | Geometry(Geometry) | Asset(AssetRef) | List(Vec<Value>)
```

- `Null` means "field unset"; materialized state omits null fields.
- `Float` must be finite (NaN/±∞ rejected with `InvalidValue`).
- `LatLon` is canonicalized on write: latitude must be within ±π/2 (else `InvalidValue`); longitude is wrapped into (−π, π]. Geometry coordinates are canonicalized the same way. (Resolves Phase-1 follow-up "canonicalize lat/lon at the Edit Log boundary".)
- `Text` ≤ 64 KiB; `List` nesting ≤ 8 levels.
- `wb-grid`/`wb-world` gain an optional `serde` feature (derives on `LatLon`, `Geometry`) used only by this crate.

### 3.3 Operations

```
Op {
  id:      OpId,
  parents: BTreeSet<OpId>,     // the committing branch's heads at commit time (empty only for the first op)
  author:  AuthorId,
  time_ms: u64,                // wall clock for display; never used for ordering
  kind:    TxKind,             // Edit | Undo(OpId) | Redo(OpId) | Restore(VersionId) | Import
  label:   String,             // "Drew mountain range" (≤ 200 bytes)
  writes:  Vec<FieldWrite>,    // non-empty, sorted by (entity, field), no duplicate (entity, field)
}
FieldWrite { entity: EntityId, field: FieldKey, value: Value }
```

One `Op` is one transaction and one undo step. Creating an entity = writes whose `entity` is the new op's own id, including `kind`. Deleting = write `deleted = Bool(true)`; undelete = write `deleted = Null`.

### 3.4 Materialization (last-writer-wins per field)

For a set of heads `H`, the reachable ops are `H` and all their ancestors. For each `(entity, field)`, the winning write is the one from the reachable op with the greatest `OpId`. `State` is:

```
State = BTreeMap<EntityId, BTreeMap<FieldKey, Value>>   // null-valued fields removed
```

Entities whose `deleted` field is `Bool(true)` remain in `State` (so undelete works) and are flagged; consumers use `State::live()` to iterate non-deleted entities. Because the winner depends only on `OpId` order, the result is independent of the order in which ops were received — this is the convergence property that makes later merge and co-editing work.

Materialization is incremental on commit (apply the new op's writes, which always win because its `lamport` is maximal) and full on load, branch switch, and version view.

### 3.5 Source hash

`SourceHash = blake3("wb-source-v1" ‖ postcard(State))`. It hashes **state, not history**: undoing back to an earlier state yields the earlier hash, so Phase-1 tile caches are reused. Assets are covered through their `AssetRef` values.

### 3.6 Validation hooks

Owning subsystems register a `Validator` per `EntityKind` (`fn validate(&EntityView) -> Result<(), String>`). On commit, every entity touched by the transaction is validated in its post-transaction state; any failure rejects the whole transaction (`ValidationFailed { entity, kind, reason }`) and leaves the log unchanged. Validators are runtime registrations, never serialized. Entities of kinds with no registered validator are accepted (the log is schema-agnostic). Undo, redo, and restore transactions are validated the same way.

## 4. History

### 4.1 Branches

- A branch is `{ name, heads: BTreeSet<OpId>, undo: Vec<OpId>, redo: Vec<OpId> }`. The log always has a current branch; a new log starts with branch `main`.
- Commit on a branch: new op's `parents = branch.heads`; afterwards `branch.heads = { new op }`.
- `fork(name, from)` creates a branch whose heads are the current branch's heads (`From::Current`) or a version's heads (`From::Version(id)`), with empty undo/redo stacks. Fails with `BranchExists` if the name is taken.
- `switch(name)` makes another branch current and re-materializes.
- Branch names: 1–64 bytes, any UTF-8 except control characters.

### 4.2 Named versions

- `save_version(name)` records `{ id, name, heads: current branch heads, branch, time_ms, author }` in metadata. No ops are copied.
- `view_version(id) -> StateView` materializes read-only at that version's heads.
- `restore_version(id)` commits one `TxKind::Restore(id)` transaction on the current branch whose writes turn the current state into the version's state (fields that differ get the version's value; fields absent in the version get `Null`). History is preserved; the restore itself is undoable.

### 4.3 Undo / redo

- Each branch keeps its own undo and redo stacks of `OpId`s.
- User transactions (`Edit`, `Import`, `Restore`) push onto `undo` and clear `redo`.
- `undo()` pops `X`, then commits `TxKind::Undo(X)` whose writes set every `(entity, field)` in `X` to its value in the state at `X.parents` (or `Null` if unset there). `X` is pushed onto `redo`.
- `redo()` pops `X`, then commits `TxKind::Redo(X)` re-writing `X`'s original values. `X` is pushed back onto `undo`.
- Undo/redo transactions never push onto the stacks themselves. `NothingToUndo` / `NothingToRedo` when empty.
- Undo applies to the latest undoable transaction on this branch only (linear undo per branch). If a later transaction on another branch touched the same fields, it is unaffected (different heads).

### 4.4 Merge (designed, not built)

Merging branch B into A will commit nothing new: A's heads become `A.heads ∪ B.heads` minus any head that is an ancestor of another. LWW materialization resolves every field. A future merge preview lists `(entity, field)` pairs whose winning value differs between the two branches' states so the user can override with a normal `Edit`. Live co-editing is the same union with ops arriving over the network. No data-model change is required for either.

### 4.5 Timeline

`timeline(branch) -> Vec<TimelineEntry>` lists the branch's reachable ops in `OpId` order with `kind`, `label`, `author`, `time_ms`, plus version labels and fork points. Undone pairs are flagged so the UI can hide them.

## 5. The `.wbworld` file

### 5.1 Layout

```
Header:  magic "WBW1" (4 bytes) · format_version u16 LE · reserved u16 = 0
         · engine_version (u8 length + UTF-8)
Sections, in this order, each:  tag [u8;4] · length u64 LE · blake3 [u8;32] · payload
  "OPS\0"   postcard(Vec<Op>) sorted by OpId
  "ASST"    postcard(Vec<(AssetRef, media_type: String, bytes: Vec<u8>)>) sorted by AssetRef
  "META"    postcard(Meta { title, authors: BTreeMap<AuthorId, String>, branches, versions,
                            current_branch, created_ms, modified_ms })
  "SNAP"    optional: postcard(Snapshot { heads, state }) for the current branch
```

- Encoding: `serde` + `postcard` 1.x. All maps are `BTreeMap`/`BTreeSet`, so bytes are deterministic across targets.
- No compression in v1.
- Format version starts at 1. Readers accept any `format_version ≤` their own and reject newer ones with `UnsupportedFormat { found }`.

### 5.2 Loading

1. Check magic, format version, and every section checksum → `CorruptFile { section }` on mismatch or truncation.
2. Decode ops; reject duplicate ids (`DuplicateOp`), unknown parents (`UnknownParent`), or ops whose `lamport` is not greater than all their parents' (`CorruptFile { section: "OPS" }`).
3. Decode assets; verify each `AssetRef` equals the blake3 of its bytes.
4. If `SNAP` is present and its heads equal the current branch's heads, use its state; otherwise (or if its state's hash disagrees with a fresh materialization in debug builds) materialize from ops.

Saving is deterministic: the same log always produces identical bytes.

### 5.3 Browser contract (implemented by Editor [10])

The editor persists each committed op as its own record (so work is never "unsaved") and uses `to_bytes()`/`from_bytes()` for export/import. The crate exposes `ops_since(heads)` and `apply_ops(ops)` for that incremental persistence.

### 5.4 Size

Typical ops are 100–300 bytes (100 k edits ≈ 10–30 MB). Assets are stored once regardless of how many branches reference them. History compaction is out of scope.

## 6. Public API

```rust
EditLog::new(actor: ActorId, author: AuthorId, clock: Box<dyn Clock>) -> EditLog
log.register_validator(kind: EntityKind, v: Box<dyn Validator>)
log.transact(label) -> Transaction            // builder
    tx.create(kind, fields) -> EntityId           // id known before commit
    tx.set(entity, field, value)
    tx.delete(entity) / tx.undelete(entity)
    tx.commit() -> Result<OpId, EditError>
log.undo() / log.redo() -> Result<OpId, EditError>
log.fork(name, From) / log.switch(name) / log.branches() / log.current_branch()
log.save_version(name) -> VersionId / log.versions()
log.view_version(id) -> Result<StateView, EditError> / log.restore_version(id) -> Result<OpId, EditError>
log.state() -> &State / log.source_hash() -> SourceHash
log.timeline(branch) -> Vec<TimelineEntry>
log.add_asset(media_type, bytes) -> AssetRef / log.asset(r) -> Option<&[u8]>
log.ops_since(heads) -> Vec<Op> / log.apply_ops(ops) -> Result<(), EditError>
log.to_bytes() -> Vec<u8> / EditLog::from_bytes(bytes, actor, author, clock) -> Result<EditLog, EditError>
```

`Clock` is a trait (`fn now_ms(&self) -> u64`) so tests are deterministic.

## 7. Errors

`EditError` (implements `Display` + `Error`; the crate never panics on user input):
`InvalidValue { field, reason }`, `InvalidKey`, `ValidationFailed { entity, kind, reason }`, `UnknownEntity(EntityId)`, `EmptyTransaction`, `UnknownParent(OpId)`, `DuplicateOp(OpId)`, `NothingToUndo`, `NothingToRedo`, `BranchExists(String)`, `UnknownBranch(String)`, `UnknownVersion(VersionId)`, `UnknownAsset(AssetRef)`, `CorruptFile { section }`, `UnsupportedFormat { found: u16 }`.

## 8. Determinism

All Phase-1 rules apply (no platform math, no FMA, sorted collections, no randomness inside the crate). `time_ms` and caller-supplied ids are data, not sources of nondeterminism. `source_hash()` and `to_bytes()` must be bit-identical on aarch64, wasm32-wasip1, and x86-64.

## 9. Testing

- **Convergence (property):** for random op sets from two simulated actors, every valid topological application order yields the same `State` and `SourceHash`.
- **Undo/redo (property):** any sequence of transactions followed by undoing all returns the initial hash; undo then redo leaves the hash unchanged; new edits clear redo.
- **Branches/versions:** forks are isolated; `restore_version` reproduces the version's hash; undoing the restore reproduces the pre-restore hash.
- **Validation:** failing validator rejects atomically (state, heads, stacks unchanged).
- **Values:** NaN/∞ rejected; longitudes wrap; out-of-range latitude rejected; key rules enforced.
- **File:** round-trip gives identical bytes and hash; each corrupted/truncated section and newer format version yields the right typed error; loading without `SNAP` gives the same state.
- **Golden:** a fixed edit script yields a fixed `SourceHash` and fixed `.wbworld` bytes (hash of bytes), checked on all three targets by `scripts/ci-local.sh`.
- **Performance guard:** loading and materializing 100 000 ops completes in < 1 s native release and < 3 s wasm32-wasip1 release on the development Mac.

## 10. Out of scope

Browser persistence code (Editor [10]), cloud sync (Services [9]), merge UI and merge operation, meaning/validation of specific entity kinds (their owning subsystems), history compaction, rich-text co-editing of lore notes.
