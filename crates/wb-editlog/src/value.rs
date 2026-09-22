use crate::error::EditError;
use crate::ids::AssetRef;
use core::cell::Cell;
use core::f64::consts::{FRAC_PI_2, PI};
use serde::{Deserialize, Serialize};
use wb_grid::LatLon;
use wb_world::Geometry;

pub const MAX_TEXT_BYTES: usize = 65_536;
pub const MAX_LIST_DEPTH: usize = 8;

thread_local! {
    static LIST_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Restores the thread-local list-nesting depth on every exit path (success,
/// error, or panic-unwind) out of `de_list`.
struct ListDepthGuard;

impl Drop for ListDepthGuard {
    fn drop(&mut self) {
        LIST_DEPTH.with(|d| d.set(d.get() - 1));
    }
}

/// Bounds `List` nesting depth *during deserialization* (not just afterwards in
/// `canonical`), so a maliciously deep `List` in a `.wbworld` file or postcard
/// payload cannot blow the stack before validation ever runs.
fn de_list<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Value>, D::Error> {
    let depth = LIST_DEPTH.with(|c| {
        let n = c.get() + 1;
        c.set(n);
        n
    });
    let _guard = ListDepthGuard;
    if depth > MAX_LIST_DEPTH {
        return Err(serde::de::Error::custom("list nesting exceeds 8 levels"));
    }
    Vec::<Value>::deserialize(d)
}

/// A field value. `Null` means "unset".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    LatLon(LatLon),
    Geometry(Geometry),
    Asset(AssetRef),
    List(#[serde(deserialize_with = "de_list")] Vec<Value>),
}

/// Wraps a longitude into (−π, π]; `-0.0` becomes `0.0` so equal points hash equally.
pub fn wrap_lon(lon: f64) -> f64 {
    let two_pi = 2.0 * PI;
    let mut x = libm::fmod(lon, two_pi);
    if x <= -PI {
        x += two_pi;
    } else if x > PI {
        x -= two_pi;
    }
    if x == 0.0 { 0.0 } else { x }
}

fn canon_latlon(p: LatLon) -> Option<LatLon> {
    if !p.lat.is_finite() || !p.lon.is_finite() || p.lat.abs() > FRAC_PI_2 {
        return None;
    }
    let lat = if p.lat == 0.0 { 0.0 } else { p.lat };
    Some(LatLon {
        lat,
        lon: wrap_lon(p.lon),
    })
}

fn canon_points(v: Vec<LatLon>) -> Option<Vec<LatLon>> {
    v.into_iter().map(canon_latlon).collect()
}

fn canon_geometry(g: Geometry) -> Option<Geometry> {
    Some(match g {
        Geometry::Point(p) => Geometry::Point(canon_latlon(p)?),
        Geometry::LineString(v) => Geometry::LineString(canon_points(v)?),
        Geometry::Polygon(v) => Geometry::Polygon(canon_points(v)?),
    })
}

impl Value {
    /// Validates and canonicalizes a value for storage; `field` names the field in errors.
    pub fn canonical(self, field: &str) -> Result<Value, EditError> {
        self.canon(field, 0)
    }

    fn canon(self, field: &str, depth: usize) -> Result<Value, EditError> {
        let bad = |reason: &str| EditError::InvalidValue {
            field: field.to_string(),
            reason: reason.to_string(),
        };
        Ok(match self {
            Value::Float(x) if !x.is_finite() => return Err(bad("float must be finite")),
            Value::Text(s) if s.len() > MAX_TEXT_BYTES => return Err(bad("text exceeds 64 KiB")),
            Value::LatLon(p) => Value::LatLon(
                canon_latlon(p).ok_or_else(|| bad("latitude out of range or non-finite"))?,
            ),
            Value::Geometry(g) => Value::Geometry(
                canon_geometry(g).ok_or_else(|| bad("geometry has an invalid coordinate"))?,
            ),
            Value::List(items) => {
                if depth >= MAX_LIST_DEPTH {
                    return Err(bad("list nesting exceeds 8 levels"));
                }
                Value::List(
                    items
                        .into_iter()
                        .map(|v| v.canon(field, depth + 1))
                        .collect::<Result<_, _>>()?,
                )
            }
            other => other,
        })
    }
}

/// True if `value` nests `List`s at most `MAX_LIST_DEPTH` deep. Iterative and
/// borrow-only, so a caller-built pathological value cannot overflow the stack here.
pub(crate) fn list_depth_ok(value: &Value) -> bool {
    let mut stack: Vec<(&Value, usize)> = vec![(value, 0)];
    while let Some((v, depth)) = stack.pop() {
        if let Value::List(items) = v {
            if depth >= MAX_LIST_DEPTH {
                return false;
            }
            stack.extend(items.iter().map(|i| (i, depth + 1)));
        }
    }
    true
}

/// True if `value` is exactly (bitwise, via its postcard encoding) its canonical form,
/// so e.g. a `-0.0` latitude is caught even though `-0.0 == 0.0`.
pub(crate) fn is_canonical(value: &Value, field: &str) -> bool {
    if !list_depth_ok(value) {
        return false;
    }
    match value.clone().canonical(field) {
        Ok(canon) => postcard::to_allocvec(&canon).ok() == postcard::to_allocvec(value).ok(),
        Err(_) => false,
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}
impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.to_string())
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}
impl From<LatLon> for Value {
    fn from(v: LatLon) -> Self {
        Value::LatLon(v)
    }
}
impl From<Geometry> for Value {
    fn from(v: Geometry) -> Self {
        Value::Geometry(v)
    }
}
impl From<AssetRef> for Value {
    fn from(v: AssetRef) -> Self {
        Value::Asset(v)
    }
}
