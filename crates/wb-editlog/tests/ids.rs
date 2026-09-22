use wb_editlog::{ActorId, EditError, EntityId, EntityKind, FieldKey, OpId};
use wb_grid::LatLon;
use wb_world::Geometry;

fn op(lamport: u64, actor: u8) -> OpId {
    OpId {
        lamport,
        actor: ActorId([actor; 16]),
    }
}

#[test]
fn op_ids_order_by_lamport_then_actor() {
    assert!(op(1, 9) < op(2, 0));
    assert!(op(3, 1) < op(3, 2));
    assert!(EntityId::PLANET < EntityId { op: op(1, 0), n: 0 });
    assert!(EntityId { op: op(4, 1), n: 0 } < EntityId { op: op(4, 1), n: 1 });
}

#[test]
fn key_rules() {
    assert!(FieldKey::new("peak_m").is_ok());
    assert!(FieldKey::new("a.b_2").is_ok());
    assert!(EntityKind::new("feature.mountain_range").is_ok());
    let long = "x".repeat(65);
    for bad in ["", "Peak", "peak m", "peak-m", long.as_str()] {
        assert_eq!(
            FieldKey::new(bad).unwrap_err(),
            EditError::InvalidKey(bad.to_string())
        );
        assert!(EntityKind::new(bad).is_err());
    }
    assert_eq!(FieldKey::new("spine").unwrap().as_str(), "spine");
    assert_eq!(FieldKey::new("spine").unwrap().to_string(), "spine");
}

#[test]
fn keys_revalidate_when_deserialized() {
    let good = postcard::to_allocvec(&FieldKey::new("ok").unwrap()).unwrap();
    assert_eq!(
        postcard::from_bytes::<FieldKey>(&good).unwrap().as_str(),
        "ok"
    );
    let forged = postcard::to_allocvec(&"NOT OK".to_string()).unwrap();
    assert!(postcard::from_bytes::<FieldKey>(&forged).is_err());
}

#[test]
fn geometry_serde_roundtrip() {
    let g = Geometry::LineString(vec![
        LatLon::from_degrees(1.0, 2.0),
        LatLon::from_degrees(3.0, 4.0),
    ]);
    let bytes = postcard::to_allocvec(&g).unwrap();
    assert_eq!(postcard::from_bytes::<Geometry>(&bytes).unwrap(), g);
}

#[test]
fn errors_display() {
    assert_eq!(EditError::NothingToUndo.to_string(), "nothing to undo");
    assert!(
        EditError::CorruptFile {
            section: "OPS".into()
        }
        .to_string()
        .contains("OPS")
    );
}
