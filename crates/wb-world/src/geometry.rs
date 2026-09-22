use crate::world::Planet;
use wb_grid::{LatLon, Vec3};

/// Angle between two points as seen from the planet's centre, in radians.
pub fn central_angle(a: LatLon, b: LatLon) -> f64 {
    let (va, vb) = (a.to_vec3(), b.to_vec3());
    libm::atan2(va.cross(vb).length(), va.dot(vb))
}

pub fn distance_m(planet: &Planet, a: LatLon, b: LatLon) -> f64 {
    central_angle(a, b) * planet.radius_m
}

pub fn polyline_length_m(planet: &Planet, pts: &[LatLon]) -> f64 {
    pts.windows(2).map(|w| distance_m(planet, w[0], w[1])).sum()
}

/// Area of a simple spherical polygon (open ring, smaller than a hemisphere).
pub fn polygon_area_m2(planet: &Planet, ring: &[LatLon]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let v: Vec<Vec3> = ring.iter().map(|p| p.to_vec3()).collect();
    let a = v[0];
    let mut excess = 0.0;
    for w in v[1..].windows(2) {
        let (b, c) = (w[0], w[1]);
        let num = a.dot(b.cross(c));
        let den = 1.0 + a.dot(b) + b.dot(c) + c.dot(a);
        excess += 2.0 * libm::atan2(num, den);
    }
    excess.abs() * planet.radius_m * planet.radius_m
}
