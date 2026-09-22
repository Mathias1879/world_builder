mod common;

use common::{ACTOR_A, AUTHOR, T0, new_log};
use std::collections::{BTreeMap, BTreeSet};
use wb_editlog::{
    Branch, EditError, EditLog, EntityId, FORMAT_VERSION, FieldKey, FieldWrite, FixedClock,
    ForkFrom, Op, OpId, SaveOptions, State, TxKind, Value, Version, VersionId,
};

fn sample() -> EditLog {
    let mut log = new_log();
    log.set_title("Aethoria");
    log.set_author_name("Matthew");
    let img = log.add_asset("image/png", vec![137, 80, 78, 71]);
    let mut tx = log.import("Imported base map");
    tx.create("import.base_map", [("image", Value::Asset(img))]);
    tx.commit().unwrap();
    let mut tx = log.transact("Drew range");
    let e = tx.create("feature.mountain_range", [("peak_m", Value::Float(5000.0))]);
    tx.commit().unwrap();
    log.save_version("v1").unwrap();
    log.fork("alt", ForkFrom::Current).unwrap();
    let mut tx = log.transact("Raise");
    tx.set(e, "peak_m", 6000.0);
    tx.commit().unwrap();
    log.undo().unwrap();
    log
}

fn load(bytes: &[u8]) -> Result<EditLog, EditError> {
    EditLog::from_bytes(bytes, ACTOR_A, AUTHOR, Box::new(FixedClock(T0)))
}

#[test]
fn roundtrip_is_byte_identical() {
    let log = sample();
    let bytes = log.to_bytes();
    assert_eq!(&bytes[..4], b"WBW1");
    assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), FORMAT_VERSION);
    let back = load(&bytes).unwrap();
    assert_eq!(back.source_hash(), log.source_hash());
    assert_eq!(back.title(), "Aethoria");
    assert_eq!(back.to_bytes(), bytes);
    assert!(back.can_redo(), "undo/redo stacks survive");
    assert_eq!(back.branches().count(), 2);
    assert_eq!(back.versions().count(), 1);
}

#[test]
fn loading_without_snapshot_gives_the_same_state() {
    let log = sample();
    let lean = log.to_bytes_with(SaveOptions { snapshot: false });
    assert!(lean.len() < log.to_bytes().len());
    let back = load(&lean).unwrap();
    assert_eq!(back.source_hash(), log.source_hash());
    assert_eq!(back.to_bytes(), log.to_bytes());
}

#[test]
fn loaded_log_keeps_editing_with_fresh_ids() {
    let log = sample();
    let mut back = load(&log.to_bytes()).unwrap();
    let v2 = back.save_version("v2").unwrap();
    assert_eq!(
        v2.seq, 1,
        "seq continues after the loading actor's last version"
    );
    let mut tx = back.transact("more");
    tx.set(wb_editlog::EntityId::PLANET, "tilt", 23.4);
    let id = tx.commit().unwrap();
    assert!(
        back.ops_since(&Default::default())
            .iter()
            .all(|o| o.id <= id)
    );
}

fn section_offset(bytes: &[u8], tag: &[u8; 4]) -> usize {
    bytes
        .windows(4)
        .position(|w| w == tag)
        .expect("section present")
}

#[test]
fn corruption_is_reported_by_section() {
    let bytes = sample().to_bytes();
    for (tag, name) in [
        (b"OPS\0", "OPS"),
        (b"ASST", "ASST"),
        (b"META", "META"),
        (b"SNAP", "SNAP"),
    ] {
        let mut bad = bytes.clone();
        let at = section_offset(&bad, tag) + 44; // first payload byte
        bad[at] ^= 0xFF;
        assert_eq!(
            load(&bad).unwrap_err(),
            EditError::CorruptFile {
                section: name.into()
            },
            "{name}"
        );
    }
    assert_eq!(
        load(&bytes[..bytes.len() - 1]).unwrap_err(),
        EditError::CorruptFile {
            section: "SNAP".into()
        }
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        load(&trailing).unwrap_err(),
        EditError::CorruptFile {
            section: "trailer".into()
        }
    );
    assert_eq!(
        load(b"NOPE").unwrap_err(),
        EditError::CorruptFile {
            section: "header".into()
        }
    );
}

#[test]
fn newer_formats_are_refused() {
    let mut bytes = sample().to_bytes();
    bytes[4..6].copy_from_slice(&(FORMAT_VERSION + 1).to_le_bytes());
    assert_eq!(
        load(&bytes).unwrap_err(),
        EditError::UnsupportedFormat {
            found: FORMAT_VERSION + 1
        }
    );
}

// --- Fix round 1 hardening: crafted-file tests below build raw section bytes for a
// minimal log (empty ops, "main" branch with empty heads) so that META's own
// consistency checks (heads/base must reference known ops) hold vacuously, letting
// each test isolate exactly one adversarial section.

const SECTION_HEADER: usize = 4 + 8 + 32;

/// Replaces one section's payload in `bytes` with `new_payload`, recomputing its
/// length and blake3 checksum, and shifting every following section accordingly.
fn replace_section(bytes: &[u8], tag: &[u8; 4], new_payload: &[u8]) -> Vec<u8> {
    let start = section_offset(bytes, tag);
    let len = u64::from_le_bytes(bytes[start + 4..start + 12].try_into().unwrap()) as usize;
    let mut out = bytes[..start].to_vec();
    out.extend_from_slice(tag);
    out.extend_from_slice(&(new_payload.len() as u64).to_le_bytes());
    out.extend_from_slice(blake3::hash(new_payload).as_bytes());
    out.extend_from_slice(new_payload);
    out.extend_from_slice(&bytes[start + SECTION_HEADER + len..]);
    out
}

fn plain_op(id: OpId, parents: BTreeSet<OpId>) -> Op {
    Op {
        id,
        parents,
        author: AUTHOR,
        time_ms: T0,
        kind: TxKind::Edit,
        label: "t".to_string(),
        writes: vec![FieldWrite {
            entity: EntityId::PLANET,
            field: FieldKey::new("tilt").unwrap(),
            value: Value::Bool(true),
        }],
    }
}

fn encode_meta(
    title: &str,
    authors: BTreeMap<wb_editlog::AuthorId, String>,
    branches: BTreeMap<String, Branch>,
    versions: BTreeMap<VersionId, Version>,
    current_branch: &str,
    created_ms: u64,
    modified_ms: u64,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(postcard::to_allocvec(&title.to_string()).unwrap());
    out.extend(postcard::to_allocvec(&authors).unwrap());
    out.extend(postcard::to_allocvec(&branches).unwrap());
    out.extend(postcard::to_allocvec(&versions).unwrap());
    out.extend(postcard::to_allocvec(&current_branch.to_string()).unwrap());
    out.extend(postcard::to_allocvec(&created_ms).unwrap());
    out.extend(postcard::to_allocvec(&modified_ms).unwrap());
    out
}

#[test]
fn duplicate_op_ids_are_rejected() {
    let base = new_log().to_bytes();
    let id = OpId {
        lamport: 1,
        actor: ACTOR_A,
    };
    let op = plain_op(id, BTreeSet::new());
    let ops = vec![op.clone(), op];
    let payload = postcard::to_allocvec(&ops).unwrap();
    let bytes = replace_section(&base, b"OPS\0", &payload);
    assert_eq!(load(&bytes).unwrap_err(), EditError::DuplicateOp(id));
}

#[test]
fn unknown_parent_is_reported() {
    let base = new_log().to_bytes();
    let missing = OpId {
        lamport: 1,
        actor: ACTOR_A,
    };
    let mut parents = BTreeSet::new();
    parents.insert(missing);
    let child = OpId {
        lamport: 2,
        actor: ACTOR_A,
    };
    let op = plain_op(child, parents);
    let payload = postcard::to_allocvec(&vec![op]).unwrap();
    let bytes = replace_section(&base, b"OPS\0", &payload);
    assert_eq!(load(&bytes).unwrap_err(), EditError::UnknownParent(missing));
}

#[test]
fn version_seq_overflow_is_corrupt_meta() {
    let base = new_log().to_bytes();
    let mut branches = BTreeMap::new();
    branches.insert("main".to_string(), Branch::default());
    let mut versions = BTreeMap::new();
    versions.insert(
        VersionId {
            actor: ACTOR_A,
            seq: u32::MAX,
        },
        Version {
            name: "v1".to_string(),
            heads: BTreeSet::new(),
            branch: "main".to_string(),
            time_ms: T0,
            author: AUTHOR,
        },
    );
    let meta = encode_meta("", BTreeMap::new(), branches, versions, "main", T0, T0);
    let bytes = replace_section(&base, b"META", &meta);
    assert_eq!(
        load(&bytes).unwrap_err(),
        EditError::CorruptFile {
            section: "META".into()
        }
    );
}

#[test]
fn invalid_branch_name_is_corrupt_meta() {
    let base = new_log().to_bytes();
    let mut branches = BTreeMap::new();
    branches.insert("main".to_string(), Branch::default());
    branches.insert("bad\u{0}name".to_string(), Branch::default());
    let meta = encode_meta(
        "",
        BTreeMap::new(),
        branches,
        BTreeMap::new(),
        "main",
        T0,
        T0,
    );
    let bytes = replace_section(&base, b"META", &meta);
    assert_eq!(
        load(&bytes).unwrap_err(),
        EditError::CorruptFile {
            section: "META".into()
        }
    );
}

#[test]
fn invalid_current_branch_name_is_corrupt_meta() {
    let base = new_log().to_bytes();
    let bad = "bad\u{0}name".to_string();
    let mut branches = BTreeMap::new();
    branches.insert(bad.clone(), Branch::default());
    let meta = encode_meta("", BTreeMap::new(), branches, BTreeMap::new(), &bad, T0, T0);
    let bytes = replace_section(&base, b"META", &meta);
    assert_eq!(
        load(&bytes).unwrap_err(),
        EditError::CorruptFile {
            section: "META".into()
        }
    );
}

#[test]
fn invalid_version_name_is_corrupt_meta() {
    let base = new_log().to_bytes();
    let mut branches = BTreeMap::new();
    branches.insert("main".to_string(), Branch::default());
    let mut versions = BTreeMap::new();
    versions.insert(
        VersionId {
            actor: ACTOR_A,
            seq: 0,
        },
        Version {
            name: "".to_string(),
            heads: BTreeSet::new(),
            branch: "main".to_string(),
            time_ms: T0,
            author: AUTHOR,
        },
    );
    let meta = encode_meta("", BTreeMap::new(), branches, versions, "main", T0, T0);
    let bytes = replace_section(&base, b"META", &meta);
    assert_eq!(
        load(&bytes).unwrap_err(),
        EditError::CorruptFile {
            section: "META".into()
        }
    );
}

/// Debug-only: release builds trust a structurally sound snapshot without re-materializing.
#[test]
#[cfg_attr(not(debug_assertions), ignore)]
fn debug_snapshot_mismatch_falls_back_to_materialized_state_instead_of_erroring() {
    let base = new_log().to_bytes();
    let mut state = State::new();
    let bogus = plain_op(
        OpId {
            lamport: 1,
            actor: ACTOR_A,
        },
        BTreeSet::new(),
    );
    state.apply(&bogus);

    let heads: BTreeSet<OpId> = BTreeSet::new();
    let mut payload = postcard::to_allocvec(&heads).unwrap();
    payload.extend(postcard::to_allocvec(&state).unwrap());
    let bytes = replace_section(&base, b"SNAP", &payload);

    let back = load(&bytes).expect("a mismatched-but-valid snapshot must not fail the load");
    assert_eq!(
        back.state().field(EntityId::PLANET, "tilt"),
        None,
        "the bogus snapshot state must be discarded in favor of materializing from ops"
    );
}

// --- Final fix 1: undo/redo stacks from a file must reference undoable ops that are
// reachable from their branch, or `undo()`/`redo()` would hit an internal `expect`.

/// A log with one edit (undone, so it sits on the redo stack) and the Undo op itself.
fn undone_log() -> EditLog {
    let mut log = new_log();
    let mut tx = log.transact("tilt");
    tx.set(EntityId::PLANET, "tilt", 1i64);
    tx.commit().unwrap();
    log.undo().unwrap();
    log
}

fn with_main_stacks(log: &EditLog, undo: Vec<OpId>, redo: Vec<OpId>) -> Vec<u8> {
    let mut branches: BTreeMap<String, Branch> = log
        .branches()
        .map(|(n, b)| (n.to_string(), b.clone()))
        .collect();
    let main = branches.get_mut("main").unwrap();
    main.undo = undo;
    main.redo = redo;
    let versions = log.versions().map(|(k, v)| (*k, v.clone())).collect();
    let meta = encode_meta(
        log.title(),
        BTreeMap::new(),
        branches,
        versions,
        "main",
        T0,
        T0,
    );
    replace_section(&log.to_bytes(), b"META", &meta)
}

fn corrupt_meta() -> EditError {
    EditError::CorruptFile {
        section: "META".into(),
    }
}

#[test]
fn undo_stack_with_unknown_op_is_corrupt_meta() {
    let ghost = OpId {
        lamport: 99,
        actor: ACTOR_A,
    };
    let bytes = with_main_stacks(&new_log(), vec![ghost], Vec::new());
    assert_eq!(load(&bytes).unwrap_err(), corrupt_meta());
}

#[test]
fn redo_stack_with_unknown_op_is_corrupt_meta() {
    let ghost = OpId {
        lamport: 99,
        actor: ACTOR_A,
    };
    let bytes = with_main_stacks(&new_log(), Vec::new(), vec![ghost]);
    assert_eq!(load(&bytes).unwrap_err(), corrupt_meta());
}

#[test]
fn undo_stack_referencing_an_undo_op_is_corrupt_meta() {
    let log = undone_log();
    let undo_op = *log.heads().iter().next().unwrap();
    assert!(matches!(log.op(undo_op).unwrap().kind, TxKind::Undo(_)));
    let bytes = with_main_stacks(&log, vec![undo_op], Vec::new());
    assert_eq!(load(&bytes).unwrap_err(), corrupt_meta());
}

#[test]
fn undo_stack_op_not_reachable_from_its_branch_is_corrupt_meta() {
    let mut log = new_log();
    log.fork("side", ForkFrom::Current).unwrap();
    log.switch("side").unwrap();
    let mut tx = log.transact("side edit");
    tx.set(EntityId::PLANET, "tilt", 1i64);
    let side_op = tx.commit().unwrap();
    log.switch("main").unwrap();
    let bytes = with_main_stacks(&log, vec![side_op], Vec::new());
    assert_eq!(load(&bytes).unwrap_err(), corrupt_meta());
}

#[test]
fn valid_undo_redo_stacks_still_load_and_work() {
    let log = undone_log();
    let mut back = load(&log.to_bytes()).unwrap();
    assert!(back.can_redo());
    back.redo().unwrap();
    back.undo().unwrap();
}

// --- Final fix 4: the header's engine version survives load/save.

/// Rewrites the header's engine-version string (the header carries no checksum).
fn with_engine_version(bytes: &[u8], engine: &[u8]) -> Vec<u8> {
    let old_len = bytes[8] as usize;
    let mut out = bytes[..8].to_vec();
    out.push(engine.len() as u8);
    out.extend_from_slice(engine);
    out.extend_from_slice(&bytes[9 + old_len..]);
    out
}

fn corrupt_header() -> EditError {
    EditError::CorruptFile {
        section: "header".into(),
    }
}

#[test]
fn engine_version_survives_load_and_save() {
    let fresh = new_log();
    assert_eq!(fresh.engine_version(), wb_world::ENGINE_VERSION);
    let crafted = with_engine_version(&fresh.to_bytes(), b"0.0.9");
    let back = load(&crafted).unwrap();
    assert_eq!(back.engine_version(), "0.0.9");
    assert_eq!(back.to_bytes(), crafted, "re-saves the exact bytes");
}

#[test]
fn non_utf8_engine_version_is_corrupt_header() {
    let crafted = with_engine_version(&new_log().to_bytes(), &[0xFF, 0xFE]);
    assert_eq!(load(&crafted).unwrap_err(), corrupt_header());
}

#[test]
fn nonzero_reserved_header_field_is_corrupt_header() {
    let mut bytes = new_log().to_bytes();
    bytes[6] = 1;
    assert_eq!(load(&bytes).unwrap_err(), corrupt_header());
}

// --- Final fix 6: a SNAP whose heads match is only trusted if it is structurally sound;
// otherwise the loader silently materializes from ops (spec §5.2).

type RawState = BTreeMap<EntityId, BTreeMap<FieldKey, Value>>;

fn pin_log() -> EditLog {
    let mut log = new_log();
    let mut tx = log.transact("pin");
    tx.create(
        "test.pin",
        [("at", Value::LatLon(wb_grid::LatLon { lat: 0.0, lon: 0.5 }))],
    );
    tx.commit().unwrap();
    log
}

fn raw_state(log: &EditLog) -> RawState {
    log.state()
        .entities()
        .map(|e| (e.id, e.fields.clone()))
        .collect()
}

fn with_snapshot(log: &EditLog, state: &RawState) -> Vec<u8> {
    let mut payload = postcard::to_allocvec(log.heads()).unwrap();
    payload.extend(postcard::to_allocvec(state).unwrap());
    replace_section(&log.to_bytes(), b"SNAP", &payload)
}

#[test]
fn snapshot_without_planet_falls_back_to_ops() {
    let log = pin_log();
    let mut state = raw_state(&log);
    state.remove(&EntityId::PLANET);
    let back = load(&with_snapshot(&log, &state)).unwrap();
    assert_eq!(
        back.state().entity(EntityId::PLANET).unwrap().kind(),
        Some("planet")
    );
    assert_eq!(back.source_hash(), log.source_hash());
}

#[test]
fn snapshot_with_null_value_falls_back_to_ops() {
    let log = pin_log();
    let mut state = raw_state(&log);
    state
        .get_mut(&EntityId::PLANET)
        .unwrap()
        .insert(FieldKey::new("tilt").unwrap(), Value::Null);
    let back = load(&with_snapshot(&log, &state)).unwrap();
    assert_eq!(back.state().field(EntityId::PLANET, "tilt"), None);
    assert_eq!(back.source_hash(), log.source_hash());
}

#[test]
fn snapshot_with_negative_zero_latitude_falls_back_to_ops() {
    let log = pin_log();
    let mut state = raw_state(&log);
    for fields in state.values_mut() {
        if let Some(Value::LatLon(p)) = fields.get_mut("at") {
            p.lat = -0.0;
        }
    }
    let back = load(&with_snapshot(&log, &state)).unwrap();
    assert_eq!(
        back.source_hash(),
        log.source_hash(),
        "-0.0 == 0.0 under PartialEq, but the snapshot is not bitwise canonical"
    );
}
