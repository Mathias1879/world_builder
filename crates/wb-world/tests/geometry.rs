use core::f64::consts::PI;
use wb_grid::LatLon;
use wb_world::{
    Extent, Feature, FeatureId, Geometry, Planet, World, distance_m, polygon_area_m2,
    polyline_length_m,
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
    assert!(
        (area - expected).abs() < 1e-9 * expected,
        "{area} vs {expected}"
    );
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
    assert_eq!(
        w.features.remove(FeatureId(3)).unwrap().kind,
        "mountain_range"
    );
    assert_eq!(w.features.len(), 2);
    assert!(w.features.get(FeatureId(3)).is_none());
}
