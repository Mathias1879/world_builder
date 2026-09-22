use proptest::prelude::*;
use wb_constraints::{bbox, bbox_contains, centroid, contains, midpoint, samples};
use wb_grid::LatLon;

fn d(lat: f64, lon: f64) -> LatLon {
    LatLon::from_degrees(lat, lon)
}

fn deg(p: LatLon) -> (f64, f64) {
    (p.lat.to_degrees(), p.lon.to_degrees())
}

#[test]
fn centroid_and_midpoint() {
    let (lat, lon) = deg(centroid(&[d(0.0, -10.0), d(0.0, 10.0)]).unwrap());
    assert!(lat.abs() < 1e-9 && lon.abs() < 1e-9);
    let (lat, _) = deg(midpoint(d(10.0, 0.0), d(30.0, 0.0)));
    assert!((lat - 20.0).abs() < 1e-9);
    assert!(centroid(&[]).is_none());
}

#[test]
fn antipodal_midpoint_is_degenerate_and_returns_the_first_point() {
    // Antipodal points have no meaningful great-circle midpoint: every point on the
    // bisecting great circle is equidistant, and `a + b` is the zero vector.
    let a = d(10.0, 20.0);
    let b = d(-10.0, -160.0);
    assert!(
        a.to_vec3().plus(b.to_vec3()).length() < 1e-12,
        "fixture must be antipodal"
    );
    let m = midpoint(a, b);
    assert!(m.lat.is_finite() && m.lon.is_finite(), "never NaN");
    assert_eq!(deg(m), deg(a));
    // The same guard protects `samples`, which midpoints consecutive vertices.
    assert!(
        samples(&[a, b], true)
            .iter()
            .all(|p| p.lat.is_finite() && p.lon.is_finite())
    );
}

#[test]
fn samples_include_midpoints() {
    let ring = [d(0.0, 0.0), d(0.0, 10.0), d(10.0, 10.0)];
    assert_eq!(samples(&ring, false).len(), 5);
    assert_eq!(samples(&ring, true).len(), 6);
    assert_eq!(samples(&[d(1.0, 1.0)], false).len(), 1);
}

#[test]
fn point_in_simple_polygon() {
    let square = [
        d(-10.0, -10.0),
        d(-10.0, 10.0),
        d(10.0, 10.0),
        d(10.0, -10.0),
    ];
    assert!(contains(&square, d(0.0, 0.0)));
    assert!(!contains(&square, d(20.0, 0.0)));
    assert!(!contains(&square, d(0.0, 170.0)));
    let reversed: Vec<LatLon> = square.iter().rev().copied().collect();
    assert!(
        contains(&reversed, d(0.0, 0.0)),
        "orientation does not matter"
    );
}

#[test]
fn polygon_across_the_antimeridian() {
    let ring = [
        d(-5.0, 170.0),
        d(-5.0, -170.0),
        d(5.0, -170.0),
        d(5.0, 170.0),
    ];
    assert!(contains(&ring, d(0.0, 180.0)));
    assert!(contains(&ring, d(0.0, -175.0)));
    assert!(!contains(&ring, d(0.0, 0.0)));
}

#[test]
fn polygon_around_a_pole() {
    let ring: Vec<LatLon> = (0..36).map(|i| d(80.0, -180.0 + 10.0 * i as f64)).collect();
    assert!(contains(&ring, d(90.0, 0.0)));
    assert!(contains(&ring, d(85.0, 123.0)));
    assert!(!contains(&ring, d(70.0, 0.0)));
}

fn cfg() -> ProptestConfig {
    ProptestConfig {
        cases: 512,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// Canonical longitude in degrees, as the Edit Log's `wrap_lon` produces: (-180, 180].
fn wrap_deg(lon: f64) -> f64 {
    let x = lon % 360.0;
    if x <= -180.0 {
        x + 360.0
    } else if x > 180.0 {
        x - 360.0
    } else {
        x
    }
}

/// An anchor in degrees, the ring's vertex offsets from it, and probe offsets.
type RingCase = (f64, f64, Vec<(f64, f64)>, Vec<(f64, f64)>);

/// A ring of vertices within ±30° of a random anchor, in canonical coordinates.
///
/// Anchored rather than uniformly random because `contains` is only defined for rings
/// smaller than a hemisphere; ±30° in each axis keeps every vertex inside a 60° cap.
/// The anchor still roams the whole globe, so poles and the antimeridian are covered.
fn anchored_ring() -> impl Strategy<Value = RingCase> + Clone {
    (
        -90.0f64..=90.0,
        -180.0f64..=180.0,
        prop::collection::vec((-30.0f64..=30.0, -30.0f64..=30.0), 3..12),
        prop::collection::vec((-45.0f64..=45.0, -45.0f64..=45.0), 1..4),
    )
}

fn at(anchor: (f64, f64), off: (f64, f64)) -> LatLon {
    d(
        (anchor.0 + off.0).clamp(-90.0, 90.0),
        wrap_deg(anchor.1 + off.1),
    )
}

proptest! {
    #![proptest_config(cfg())]

    /// The prefilter's whole contract: it may let extra points through, but it must
    /// never reject one that `contains` accepts.
    #[test]
    fn bbox_never_rejects_a_point_contains_accepts(
        (c_lat, c_lon, ring_off, probe_off) in anchored_ring(),
        far_lat in -90.0f64..=90.0,
        far_lon in -180.0f64..=180.0,
    ) {
        let anchor = (c_lat, c_lon);
        let ring: Vec<LatLon> = ring_off.iter().map(|&o| at(anchor, o)).collect();
        let b = bbox(&ring).expect("3 or more points");
        let mut probes: Vec<LatLon> = probe_off.iter().map(|&o| at(anchor, o)).collect();
        probes.push(d(far_lat, wrap_deg(far_lon)));
        for p in probes {
            if contains(&ring, p) {
                prop_assert!(
                    bbox_contains(b, p),
                    "bbox {b:?} rejected {p:?}, which is inside {ring:?}"
                );
            }
        }
    }

    /// Degenerate rings have no box, exactly as they have no inside.
    #[test]
    fn bbox_is_none_for_short_rings(
        ring_deg in prop::collection::vec((-90.0f64..=90.0, -179.999f64..=180.0), 0..3),
    ) {
        let ring: Vec<LatLon> = ring_deg.iter().map(|&(a, b)| d(a, b)).collect();
        prop_assert!(bbox(&ring).is_none());
        prop_assert!(!contains(&ring, d(0.0, 0.0)));
    }
}
