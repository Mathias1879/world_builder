use wb_constraints::{centroid, contains, midpoint, samples};
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
