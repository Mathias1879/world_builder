//! Cross-target determinism gate for the edit log. If this fails after an intentional
//! change to encoding or semantics, update both constants in the same commit.
//!
//! `GOLDEN_FILE` hashes the whole `.wbworld` file, whose header embeds
//! `wb_world::ENGINE_VERSION` (the crate version): bump `GOLDEN_FILE` whenever the crate
//! version changes. `GOLDEN_SOURCE` hashes state only and is unaffected.

mod common;

use common::new_log;
use wb_editlog::{EditLog, EntityId, ForkFrom, Value};
use wb_grid::LatLon;
use wb_world::{Geometry, to_hex};

const GOLDEN_SOURCE: &str = "8e5178086c8361d68c7e3f5634a7b45e680e09524564928689f6af8f84db17a1";
const GOLDEN_FILE: &str = "ea6d8d5c285b54cfb5dea504226c6290be6e8761809eb90260ce1a7f8cbad745";

fn script() -> EditLog {
    let mut log = new_log();
    log.set_title("Golden");
    log.set_author_name("golden");
    let img = log.add_asset("image/png", (0u8..=255).collect());
    let mut tx = log.import("Base map");
    tx.create("import.base_map", [("image", Value::Asset(img))]);
    tx.commit().unwrap();
    let mut tx = log.transact("Range");
    let range = tx.create(
        "feature.mountain_range",
        [
            (
                "spine",
                Value::Geometry(Geometry::LineString(vec![
                    LatLon::from_degrees(10.0, 200.0),
                    LatLon::from_degrees(12.5, 20.0),
                ])),
            ),
            ("peak_m", Value::Float(5123.25)),
        ],
    );
    tx.set(EntityId::PLANET, "axial_tilt_deg", 23.44);
    tx.commit().unwrap();
    let v = log.save_version("v1").unwrap();
    let mut tx = log.transact("Raise");
    tx.set(range, "peak_m", 7000.5);
    tx.commit().unwrap();
    log.undo().unwrap();
    log.redo().unwrap();
    log.fork("alt", ForkFrom::Version(v)).unwrap();
    log.switch("alt").unwrap();
    let mut tx = log.transact("City");
    tx.create(
        "civ.settlement",
        [
            ("name", Value::from("Kaldros")),
            ("at", Value::LatLon(LatLon::from_degrees(-5.0, -179.0))),
        ],
    );
    tx.commit().unwrap();
    log.switch("main").unwrap();
    log.restore_version(v).unwrap();
    log
}

#[test]
fn golden_edit_log() {
    let log = script();
    let source = to_hex(&log.source_hash().0);
    let file = to_hex(blake3::hash(&log.to_bytes()).as_bytes());
    assert_eq!(
        source, GOLDEN_SOURCE,
        "source hash changed; if intentional, set GOLDEN_SOURCE = \"{source}\""
    );
    assert_eq!(
        file, GOLDEN_FILE,
        "file bytes changed; if intentional, set GOLDEN_FILE = \"{file}\""
    );
}
