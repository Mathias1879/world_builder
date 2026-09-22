use core::f64::consts::{FRAC_PI_2, PI};
use core::fmt;
use std::collections::BTreeSet;
use wb_grid::{CellId, Face, LatLon, MAX_LEVEL, TileId, Vec3, face_to_sphere, tiles_per_edge};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtentError {
    LatitudeOrder,
    LatitudeRange,
    LongitudeRange,
    ZeroWidth,
}

impl fmt::Display for ExtentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            ExtentError::LatitudeOrder => "south must be less than north",
            ExtentError::LatitudeRange => "latitudes must be within ±90°",
            ExtentError::LongitudeRange => "longitudes must be within ±180°",
            ExtentError::ZeroWidth => "west and east must differ",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for ExtentError {}

/// Latitude/longitude box in radians. `west > east` crosses the antimeridian.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionBox {
    pub south: f64,
    pub north: f64,
    pub west: f64,
    pub east: f64,
}

fn wrap_lon(x: f64) -> f64 {
    if x > PI {
        x - 2.0 * PI
    } else if x <= -PI {
        x + 2.0 * PI
    } else {
        x
    }
}

impl RegionBox {
    pub fn new(south: f64, north: f64, west: f64, east: f64) -> Result<Self, ExtentError> {
        if !(-FRAC_PI_2..=FRAC_PI_2).contains(&south) || !(-FRAC_PI_2..=FRAC_PI_2).contains(&north)
        {
            return Err(ExtentError::LatitudeRange);
        }
        if south >= north {
            return Err(ExtentError::LatitudeOrder);
        }
        if !(-PI..=PI).contains(&west) || !(-PI..=PI).contains(&east) {
            return Err(ExtentError::LongitudeRange);
        }
        // `west = 180°, east = −180°` names one meridian twice: zero width.
        // (`west = −180°, east = 180°` is the full circle and stays valid.)
        if west == east || (west == PI && east == -PI) {
            return Err(ExtentError::ZeroWidth);
        }
        Ok(Self {
            south,
            north,
            west,
            east,
        })
    }

    pub fn from_degrees(south: f64, north: f64, west: f64, east: f64) -> Result<Self, ExtentError> {
        Self::new(
            south.to_radians(),
            north.to_radians(),
            west.to_radians(),
            east.to_radians(),
        )
    }

    pub fn contains(&self, p: LatLon) -> bool {
        if p.lat < self.south || p.lat > self.north {
            return false;
        }
        if self.west <= self.east {
            p.lon >= self.west && p.lon <= self.east
        } else {
            p.lon >= self.west || p.lon <= self.east
        }
    }

    /// Corners, edge midpoints, and centre.
    pub fn sample_points(&self) -> [LatLon; 9] {
        let span = self.lon_span();
        let lons = [self.west, wrap_lon(self.west + span / 2.0), self.east];
        let lats = [self.south, (self.south + self.north) / 2.0, self.north];
        let mut out = [LatLon { lat: 0.0, lon: 0.0 }; 9];
        for (k, (lat, lon)) in lats
            .iter()
            .flat_map(|a| lons.iter().map(move |o| (*a, *o)))
            .enumerate()
        {
            out[k] = LatLon { lat, lon };
        }
        out
    }

    /// Longitude span of the box, in `(0, 2π]`.
    fn lon_span(&self) -> f64 {
        if self.west <= self.east {
            self.east - self.west
        } else {
            self.east - self.west + 2.0 * PI
        }
    }

    /// Conservative: true whenever the tile could intersect the box (it may
    /// also be true for a tile that only comes within a hair of it).
    fn touches(&self, t: TileId) -> bool {
        let b = TileBounds::of(t);
        if b.lat_max < self.south || b.lat_min > self.north {
            return false;
        }
        if b.lon_width >= 2.0 * PI {
            return true;
        }
        let span = self.lon_span();
        in_arc(b.lon_start, self.west, span) || in_arc(self.west, b.lon_start, b.lon_width)
    }
}

/// True if longitude `x` lies within `[start, start + width]` on the circle.
fn in_arc(x: f64, start: f64, width: f64) -> bool {
    (x - start).rem_euclid(2.0 * PI) <= width
}

/// Padding added to computed tile bounds to absorb rounding (≈ 6 mm on Earth).
const PAD: f64 = 1e-9;

/// Conservative latitude/longitude bounding box of a tile. Tile edges are
/// great-circle arcs (lines of constant equi-angular `s` or `t`).
struct TileBounds {
    lat_min: f64,
    lat_max: f64,
    /// Westernmost longitude; the box runs east from here by `lon_width`.
    lon_start: f64,
    lon_width: f64,
}

impl TileBounds {
    fn of(t: TileId) -> TileBounds {
        let n = tiles_per_edge(t.level);
        let polar = matches!(t.face, Face::PosZ | Face::NegZ);
        // Grid-vertex coordinates of the corners, in boundary order. The face
        // centre (a pole on PosZ/NegZ) is the vertex where 2·x == n.
        let corners = [
            (t.x, t.y),
            (t.x + 1, t.y),
            (t.x + 1, t.y + 1),
            (t.x, t.y + 1),
        ];
        let is_pole = |(gx, gy): (u32, u32)| polar && 2 * gx == n && 2 * gy == n;
        let st = |g: u32| -1.0 + 2.0 * g as f64 / n as f64;
        let pts = corners.map(|(gx, gy)| face_to_sphere(t.face, st(gx), st(gy)));

        let mut lat_min = f64::INFINITY;
        let mut lat_max = f64::NEG_INFINITY;
        for k in 0..4 {
            let (a, b) = (pts[k], pts[(k + 1) % 4]);
            for p in [a, b] {
                let lat = LatLon::from_vec3(p).lat;
                lat_min = lat_min.min(lat);
                lat_max = lat_max.max(lat);
            }
            for lat in arc_lat_extrema(a, b) {
                lat_min = lat_min.min(lat);
                lat_max = lat_max.max(lat);
            }
        }

        // Pole strictly inside the tile (only the level-0 polar faces).
        let interior_pole = polar && n == 1;
        if interior_pole {
            let (lat_min, lat_max) = if t.face == Face::PosZ {
                (lat_min, FRAC_PI_2)
            } else {
                (-FRAC_PI_2, lat_max)
            };
            return TileBounds {
                lat_min: lat_min - PAD,
                lat_max: lat_max + PAD,
                lon_start: -PI,
                lon_width: 2.0 * PI,
            };
        }

        // No pole inside: longitude is monotonic along each edge and every edge
        // sweeps less than π, so the corners' unwrapped longitudes bound it. A
        // corner at a pole has no longitude; both edges meeting there are
        // meridians meeting at 90°, so the walk from one neighbour of the pole
        // corner to the other takes the short way round, through the tile.
        // (The pole corner's latitude, ±90°, is already in the lat range.)
        let lons: Vec<f64> = (0..4)
            .filter(|&k| !is_pole(corners[k]))
            .map(|k| LatLon::from_vec3(pts[k]).lon)
            .collect();
        let (mut lo, mut hi, mut cur) = (0.0f64, 0.0f64, 0.0f64);
        for w in lons.windows(2) {
            cur += wrap_lon(w[1] - w[0]);
            lo = lo.min(cur);
            hi = hi.max(cur);
        }
        TileBounds {
            lat_min: lat_min - PAD,
            lat_max: lat_max + PAD,
            lon_start: lons[0] + lo - PAD,
            lon_width: hi - lo + 2.0 * PAD,
        }
    }
}

/// Latitudes of the northernmost/southernmost points of the great circle
/// through `a` and `b` that lie strictly within the minor arc `a`→`b`.
fn arc_lat_extrema(a: Vec3, b: Vec3) -> Vec<f64> {
    let normal = a.cross(b);
    let len = normal.length();
    if len == 0.0 {
        return Vec::new();
    }
    let nn = normal.scaled(1.0 / len);
    // Highest point of the circle: z projected onto the circle's plane.
    let z = Vec3::new(0.0, 0.0, 1.0);
    let top = z.plus(nn.scaled(-nn.z));
    if top.length() == 0.0 {
        return Vec::new(); // equator-parallel plane: circle is the equator
    }
    let top = top.normalize();
    let mut out = Vec::new();
    for p in [top, top.scaled(-1.0)] {
        if a.cross(p).dot(normal) > 0.0 && p.cross(b).dot(normal) > 0.0 {
            out.push(LatLon::from_vec3(p).lat);
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Extent {
    Planet,
    Region(RegionBox),
}

impl Extent {
    pub fn contains(&self, p: LatLon) -> bool {
        match self {
            Extent::Planet => true,
            Extent::Region(r) => r.contains(p),
        }
    }

    /// Tiles at `level` covering this extent, sorted.
    pub fn tiles(&self, level: u8) -> Vec<TileId> {
        assert!(level <= MAX_LEVEL, "level {level} exceeds MAX_LEVEL");
        let r = match self {
            Extent::Planet => return TileId::all(level),
            Extent::Region(r) => r,
        };
        let samples = r.sample_points();
        let located = |l: u8| samples.map(|p| CellId::locate(p.to_vec3(), l).tile().0);
        let mut current: BTreeSet<TileId> = TileId::all(0)
            .into_iter()
            .filter(|t| r.touches(*t))
            .collect();
        current.extend(located(0));
        for l in 1..=level {
            let mut next = BTreeSet::new();
            for t in &current {
                for c in t.children().expect("level <= MAX_LEVEL") {
                    if r.touches(c) {
                        next.insert(c);
                    }
                }
            }
            next.extend(located(l));
            current = next;
        }
        current.into_iter().collect()
    }
}
