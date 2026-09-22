mod common;

use common::{ACTOR_A, AUTHOR, T0, new_log};
use wb_editlog::{EditError, EditLog, FORMAT_VERSION, FixedClock, ForkFrom, SaveOptions, Value};

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
