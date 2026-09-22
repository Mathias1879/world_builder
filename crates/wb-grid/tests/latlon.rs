use proptest::prelude::*;
use wb_grid::{LatLon, Vec3};

fn close(a: Vec3, b: Vec3, eps: f64) -> bool {
    (a.x - b.x).abs() < eps && (a.y - b.y).abs() < eps && (a.z - b.z).abs() < eps
}

#[test]
fn cardinal_points() {
    assert!(close(
        LatLon::from_degrees(0.0, 0.0).to_vec3(),
        Vec3::new(1.0, 0.0, 0.0),
        1e-15
    ));
    assert!(close(
        LatLon::from_degrees(0.0, 90.0).to_vec3(),
        Vec3::new(0.0, 1.0, 0.0),
        1e-15
    ));
    assert!(close(
        LatLon::from_degrees(90.0, 0.0).to_vec3(),
        Vec3::new(0.0, 0.0, 1.0),
        1e-15
    ));
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
    ProptestConfig {
        cases: 256,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
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
