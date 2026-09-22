use core::f64::consts::PI;
use wb_grid::{LatLon, Vec3};

/// Spherical centroid: normalized sum of unit vectors. `None` if empty or degenerate.
pub fn centroid(points: &[LatLon]) -> Option<LatLon> {
    let sum = points
        .iter()
        .fold(Vec3::new(0.0, 0.0, 0.0), |acc, p| acc.plus(p.to_vec3()));
    if points.is_empty() || sum.length() < 1e-12 {
        return None;
    }
    Some(LatLon::from_vec3(sum.normalize()))
}

/// Great-circle midpoint.
pub fn midpoint(a: LatLon, b: LatLon) -> LatLon {
    LatLon::from_vec3(a.to_vec3().plus(b.to_vec3()).normalize())
}

/// Vertices followed by segment midpoints (and the closing edge's midpoint if `closed`).
pub fn samples(points: &[LatLon], closed: bool) -> Vec<LatLon> {
    let mut out = points.to_vec();
    for w in points.windows(2) {
        out.push(midpoint(w[0], w[1]));
    }
    if closed && points.len() > 2 {
        out.push(midpoint(points[points.len() - 1], points[0]));
    }
    out
}

/// Winding test on the sphere: sum the signed angles each edge subtends around `p`
/// (edges projected onto the tangent plane at `p`); inside when |sum| > π.
pub fn contains(ring: &[LatLon], p: LatLon) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let pv = p.to_vec3();

    // Check if point is too far from the ring (more than 90 degrees from any vertex)
    let mut min_dot: f64 = -1.0;
    for q in ring {
        let qv = q.to_vec3();
        min_dot = min_dot.max(qv.dot(pv));
    }
    // If dot product is negative, angle is > 90 degrees
    if min_dot < 0.0 {
        return false;
    }

    let tangent = |q: LatLon| {
        let v = q.to_vec3();
        v.plus(pv.scaled(-v.dot(pv)))
    };
    let mut total = 0.0;
    for (i, q) in ring.iter().enumerate() {
        let a = tangent(*q);
        let b = tangent(ring[(i + 1) % ring.len()]);
        total += libm::atan2(pv.dot(a.cross(b)), a.dot(b));
    }
    total.abs() > PI
}
