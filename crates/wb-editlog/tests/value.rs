use core::f64::consts::{FRAC_PI_2, PI};
use wb_editlog::{EditError, MAX_TEXT_BYTES, Value, wrap_lon};
use wb_grid::LatLon;
use wb_world::Geometry;

fn ll(lat: f64, lon: f64) -> LatLon {
    LatLon { lat, lon }
}

#[test]
fn longitudes_wrap_into_half_open_range() {
    assert_eq!(wrap_lon(0.5), 0.5);
    assert_eq!(wrap_lon(PI), PI);
    assert_eq!(wrap_lon(-PI), PI);
    assert!((wrap_lon(3.0 * PI / 2.0) - (-PI / 2.0)).abs() < 1e-15);
    assert!((wrap_lon(-3.0 * PI / 2.0) - (PI / 2.0)).abs() < 1e-15);
    assert_eq!(wrap_lon(-0.0).to_bits(), 0.0f64.to_bits());
}

#[test]
fn latlon_canonicalizes() {
    let v = Value::LatLon(ll(0.3, 2.0 * PI + 0.25))
        .canonical("pos")
        .unwrap();
    match v {
        Value::LatLon(p) => {
            assert_eq!(p.lat, 0.3);
            assert!((p.lon - 0.25).abs() < 1e-15);
        }
        other => panic!("unexpected {other:?}"),
    }
    let bad = Value::LatLon(ll(FRAC_PI_2 + 0.01, 0.0))
        .canonical("pos")
        .unwrap_err();
    assert!(matches!(bad, EditError::InvalidValue { ref field, .. } if field == "pos"));
    assert!(Value::LatLon(ll(f64::NAN, 0.0)).canonical("pos").is_err());
}

#[test]
fn geometry_canonicalizes_every_point() {
    let g = Geometry::Polygon(vec![ll(0.0, 4.0), ll(0.1, -4.0), ll(0.2, 0.0)]);
    let Value::Geometry(Geometry::Polygon(pts)) = Value::Geometry(g).canonical("area").unwrap()
    else {
        panic!("shape changed")
    };
    assert!(pts.iter().all(|p| p.lon > -PI && p.lon <= PI));
    let bad = Geometry::Point(ll(2.0, 0.0));
    assert!(Value::Geometry(bad).canonical("at").is_err());
}

#[test]
fn floats_must_be_finite() {
    assert!(Value::Float(1.5).canonical("x").is_ok());
    for f in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(Value::Float(f).canonical("x").is_err());
    }
    assert!(
        Value::List(vec![Value::Float(f64::NAN)])
            .canonical("x")
            .is_err()
    );
}

#[test]
fn text_and_nesting_limits() {
    assert!(
        Value::Text("a".repeat(MAX_TEXT_BYTES))
            .canonical("t")
            .is_ok()
    );
    assert!(
        Value::Text("a".repeat(MAX_TEXT_BYTES + 1))
            .canonical("t")
            .is_err()
    );
    let mut v = Value::Int(1);
    for _ in 0..8 {
        v = Value::List(vec![v]);
    }
    assert!(v.clone().canonical("l").is_ok());
    assert!(Value::List(vec![v]).canonical("l").is_err());
}

#[test]
fn list_depth_eight_round_trips_nine_fails_to_decode() {
    let mut v = Value::Int(1);
    for _ in 0..8 {
        v = Value::List(vec![v]);
    }
    let bytes = postcard::to_allocvec(&v).unwrap();
    let back: Value = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(back, v);

    let too_deep = Value::List(vec![v]);
    let bytes = postcard::to_allocvec(&too_deep).unwrap();
    assert!(postcard::from_bytes::<Value>(&bytes).is_err());
}

#[test]
fn pathologically_deep_list_bytes_are_rejected_without_crashing() {
    // Derive the List tag/length prefix from a real encoding instead of hard-coding
    // it, so this test still means what it says if the wire format ever changes.
    let sample = postcard::to_allocvec(&Value::List(vec![Value::Null])).unwrap();
    let (prefix, suffix) = sample.split_at(sample.len() - 1);
    assert_eq!(prefix, [8u8, 1u8], "List tag/length encoding changed");
    assert_eq!(suffix, [0u8], "Null tag encoding changed");

    let mut bytes = prefix.repeat(200_000);
    bytes.extend_from_slice(suffix);
    assert!(postcard::from_bytes::<Value>(&bytes).is_err());
}

#[test]
fn conversions() {
    assert_eq!(Value::from(true), Value::Bool(true));
    assert_eq!(Value::from(3i64), Value::Int(3));
    assert_eq!(Value::from(2.5), Value::Float(2.5));
    assert_eq!(Value::from("hi"), Value::Text("hi".into()));
}

#[test]
fn entity_references_are_values() {
    let e = wb_editlog::EntityId {
        op: wb_editlog::OpId {
            lamport: 3,
            actor: wb_editlog::ActorId([2; 16]),
        },
        n: 1,
    };
    let v = Value::from(e);
    assert_eq!(v, Value::Entity(e));
    assert_eq!(v.clone().canonical("subject").unwrap(), v);
    let bytes = postcard::to_allocvec(&v).unwrap();
    assert_eq!(postcard::from_bytes::<Value>(&bytes).unwrap(), v);
}
