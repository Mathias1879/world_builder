#![allow(dead_code)]
use wb_constraints::{Terrain, register_kinds};
use wb_editlog::{ActorId, AuthorId, EditLog, EntityId, FixedClock, Value};
use wb_grid::LatLon;
use wb_world::Geometry;

pub fn new_log() -> EditLog {
    let mut log = EditLog::new(ActorId([1; 16]), AuthorId([9; 16]), Box::new(FixedClock(1_758_000_000_000)));
    register_kinds(&mut log);
    log
}

pub fn add(log: &mut EditLog, kind: &str, fields: Vec<(&str, Value)>) -> EntityId {
    let mut tx = log.transact("add");
    let e = tx.create(kind, fields);
    tx.commit().unwrap();
    e
}

pub fn d(lat: f64, lon: f64) -> LatLon {
    LatLon::from_degrees(lat, lon)
}

pub fn poly(pts: &[(f64, f64)]) -> Value {
    Value::Geometry(Geometry::Polygon(pts.iter().map(|&(a, b)| d(a, b)).collect()))
}

pub fn line(pts: &[(f64, f64)]) -> Value {
    Value::Geometry(Geometry::LineString(pts.iter().map(|&(a, b)| d(a, b)).collect()))
}

/// Lat/lon box as a polygon with a vertex every 1° along each edge (close to a true lat/lon box).
pub fn band(s: f64, n: f64, w: f64, e: f64) -> Value {
    let mut pts = Vec::new();
    let steps = |a: f64, b: f64| ((b - a).abs().ceil() as usize).max(1);
    let (ns, nw) = (steps(s, n), steps(w, e));
    for i in 0..nw {
        pts.push((s, w + (e - w) * i as f64 / nw as f64));
    }
    for i in 0..ns {
        pts.push((s + (n - s) * i as f64 / ns as f64, e));
    }
    for i in 0..nw {
        pts.push((n, e - (e - w) * i as f64 / nw as f64));
    }
    for i in 0..ns {
        pts.push((n - (n - s) * i as f64 / ns as f64, w));
    }
    poly(&pts)
}

/// Land where longitude < 0, ocean elsewhere.
pub struct WestIsLand;

impl Terrain for WestIsLand {
    fn is_land(&self, p: LatLon) -> Option<bool> {
        Some(p.lon < 0.0)
    }
}
