use core::f64::consts::{FRAC_PI_2, PI};
use core::fmt;
use std::collections::BTreeSet;
use wb_grid::{CellId, LatLon, MAX_LEVEL, TileId, face_to_sphere, tiles_per_edge};

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
        if west == east {
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
        let span = if self.west <= self.east {
            self.east - self.west
        } else {
            self.east - self.west + 2.0 * PI
        };
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

    fn touches(&self, t: TileId) -> bool {
        const K: u32 = 16;
        let n = tiles_per_edge(t.level) as f64;
        (0..=K).any(|a| {
            (0..=K).any(|b| {
                let s = -1.0 + 2.0 * (t.x as f64 + a as f64 / K as f64) / n;
                let tt = -1.0 + 2.0 * (t.y as f64 + b as f64 / K as f64) / n;
                self.contains(LatLon::from_vec3(face_to_sphere(t.face, s, tt)))
            })
        })
    }
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
