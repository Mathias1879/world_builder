use core::f64::consts::{FRAC_PI_2, PI};
use wb_grid::{LatLon, Vec3};

/// Degenerate below this vector length: the summed direction carries no information.
const DEGENERATE: f64 = 1e-12;

/// Spherical centroid: the normalized sum of the points' unit vectors.
///
/// Returns `None` when the input is empty, or when the sum is degenerate — its length
/// is below `1e-12` — which happens for antipodal or otherwise symmetric point sets
/// (e.g. `[(0, 0), (0, 180)]`, or a ring evenly spread around the whole sphere). No
/// direction is more central than any other in those cases, so there is no answer to
/// give. Never panics.
pub fn centroid(points: &[LatLon]) -> Option<LatLon> {
    let sum = points
        .iter()
        .fold(Vec3::new(0.0, 0.0, 0.0), |acc, p| acc.plus(p.to_vec3()));
    if points.is_empty() || sum.length() < DEGENERATE {
        return None;
    }
    Some(LatLon::from_vec3(sum.normalize()))
}

/// The great-circle midpoint of `a` and `b`: `normalize(a + b)` as unit vectors.
///
/// Antipodal input has no meaningful midpoint — every point on the bisecting great
/// circle is equidistant, and `a + b` is the zero vector. When the sum's length is
/// below `1e-12` this returns `a` unchanged rather than a normalized rounding error.
/// Never panics and never returns NaN.
pub fn midpoint(a: LatLon, b: LatLon) -> LatLon {
    let sum = a.to_vec3().plus(b.to_vec3());
    if sum.length() < DEGENERATE {
        return a;
    }
    LatLon::from_vec3(sum.normalize())
}

/// Every vertex, followed by the [`midpoint`] of each consecutive pair.
///
/// With `closed`, the midpoint of the closing edge (last → first) is appended too, but
/// only for rings of 3 or more points: for 2 points that edge duplicates the one
/// already sampled, and for 1 point it does not exist. So the output length is
/// `2n − 1` for an open run of `n ≥ 1` points, `2n` for a closed ring of `n ≥ 3`.
/// Never panics; an empty input yields an empty output.
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

/// Is `p` inside the spherical polygon `ring`?
///
/// Winding test on the sphere: sum the signed angles each edge subtends around `p`
/// (edges projected onto the tangent plane at `p`); inside when |sum| > π. The ring is
/// implicitly closed (last vertex → first) and orientation does not matter.
///
/// **Precondition: the ring must be smaller than a hemisphere.** A larger ring has no
/// unambiguous inside — the winding test would report its complement just as
/// plausibly. This returns `false` (never an error, never a panic) for any input
/// outside that contract, and `false` for a ring of fewer than 3 points.
pub fn contains(ring: &[LatLon], p: LatLon) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let pv = p.to_vec3();

    // Cheap reject: within the hemisphere contract, a point more than 90° from EVERY
    // vertex cannot be inside. `max_dot` is the dot product with the nearest vertex;
    // negative means that nearest vertex is already more than 90° away.
    let mut max_dot: f64 = -1.0;
    for q in ring {
        let qv = q.to_vec3();
        max_dot = max_dot.max(qv.dot(pv));
    }
    if max_dot < 0.0 {
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

/// Latitude/longitude bounding box of a ring, in radians: `(min_lat, max_lat, min_lon,
/// max_lon)`. `None` for fewer than 3 points, matching [`contains`].
///
/// This is a **prefilter**, not a second point-in-polygon test. Its one guarantee is
/// conservatism: whenever `contains(ring, p)` is true, `bbox_contains(bbox(ring)?, p)`
/// is also true. It may well answer true for points `contains` rejects — that costs a
/// winding test, not a wrong verdict. A proptest over random rings and points pins the
/// implication down.
///
/// Two spherical subtleties make the naive vertex min/max unsound, and both are
/// handled here:
///
/// - **Great-circle edges bulge poleward** of their endpoints, so a point can be inside
///   the ring at a higher latitude than any vertex. Latitude is 1-Lipschitz in angular
///   distance and every point of an edge lies within half that edge's length of an
///   endpoint, so the latitude bounds are padded by half the longest edge. The edge
///   length is bounded above by `|Δlat| + |Δlon|` — the lat-then-lon detour is never
///   shorter than the geodesic — which keeps the prefilter free of transcendental math.
/// - **Rings that wrap.** If the vertex longitudes span more than π, the ring either
///   crosses the antimeridian or encloses a pole; in the pole case its latitudes reach
///   beyond every vertex's, no matter how the box is padded. Rather than guess which,
///   the box degrades to the whole sphere, so the prefilter simply stops filtering for
///   those rings instead of rejecting a true hit.
///
/// Assumes canonical coordinates — `lat` in [−π/2, π/2], `lon` in (−π, π] — as the Edit
/// Log's value canonicalization produces. Never panics.
pub fn bbox(ring: &[LatLon]) -> Option<(f64, f64, f64, f64)> {
    if ring.len() < 3 {
        return None;
    }
    let mut min_lat = f64::INFINITY;
    let mut max_lat = f64::NEG_INFINITY;
    let mut min_lon = f64::INFINITY;
    let mut max_lon = f64::NEG_INFINITY;
    for p in ring {
        min_lat = min_lat.min(p.lat);
        max_lat = max_lat.max(p.lat);
        min_lon = min_lon.min(p.lon);
        max_lon = max_lon.max(p.lon);
    }
    if !(min_lat.is_finite() && max_lat.is_finite() && min_lon.is_finite() && max_lon.is_finite()) {
        return Some((-FRAC_PI_2, FRAC_PI_2, -PI, PI));
    }
    if max_lon - min_lon > PI {
        return Some((-FRAC_PI_2, FRAC_PI_2, -PI, PI));
    }
    let mut pad: f64 = 0.0;
    for (i, a) in ring.iter().enumerate() {
        let b = ring[(i + 1) % ring.len()];
        pad = pad.max(((a.lat - b.lat).abs() + (a.lon - b.lon).abs()) * 0.5);
    }
    Some((
        (min_lat - pad).max(-FRAC_PI_2),
        (max_lat + pad).min(FRAC_PI_2),
        min_lon,
        max_lon,
    ))
}

/// Is `p` inside the box `(min_lat, max_lat, min_lon, max_lon)` from [`bbox`]?
/// Bounds are inclusive. Never panics.
pub fn bbox_contains(b: (f64, f64, f64, f64), p: LatLon) -> bool {
    let (min_lat, max_lat, min_lon, max_lon) = b;
    p.lat >= min_lat && p.lat <= max_lat && p.lon >= min_lon && p.lon <= max_lon
}
