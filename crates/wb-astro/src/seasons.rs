//! Reading seasons off the sunlight curve.
//!
//! The user owns their seasons: any number, any lengths, their own names. This only
//! offers a starting point — where the light turns — as suggestions they apply and then
//! rename.

use crate::sky::SkyModel;
use std::collections::BTreeMap;
use wb_constraints::{IssueCode, NewEntity, Param, Suggestion};
use wb_editlog::Value;
use wb_grid::LatLon;

/// The latitude the curve is read at. Mid-latitudes have the clearest seasons: the
/// tropics barely vary and a pole is all-or-nothing.
pub const PROPOSAL_LAT_DEG: f64 = 45.0;

/// Days of the year where the curve turns: local extrema, and the steepest day of each
/// arc between them. Circular — day 0 follows the last day.
fn turning_points(curve: &[f64]) -> Vec<i64> {
    let n = curve.len();
    if n < 3 {
        return Vec::new();
    }
    let at = |i: usize| curve[i % n];
    let slope = |i: usize| at(i + 1) - at(i + n - 1);
    let mut extrema = Vec::new();
    for i in 0..n {
        let before = at(i) - at(i + n - 1);
        let after = at(i + 1) - at(i);
        if (before > 0.0 && after <= 0.0) || (before < 0.0 && after >= 0.0) {
            extrema.push(i as i64);
        }
    }
    let mut points = extrema.clone();
    // The steepest day of each arc between neighbouring extrema, including the arc that
    // wraps the year end — without that last one, the steepest day near day 0 is never a
    // candidate and a cut there falls back to even spacing instead of a turning point.
    if extrema.len() >= 2 {
        for j in 0..extrema.len() {
            let a = extrema[j] as usize;
            let b = if j + 1 < extrema.len() {
                extrema[j + 1] as usize
            } else {
                extrema[0] as usize + n
            };
            let mut best = (f64::MIN, a);
            for i in a..=b {
                let s = slope(i).abs();
                if s > best.0 {
                    best = (s, i % n);
                }
            }
            points.push(best.1 as i64);
        }
    }
    points.sort_unstable();
    points.dedup();
    points
}

/// `n` strictly increasing cut days in `0..curve.len()`.
///
/// Each cut is the turning point nearest its evenly spaced ideal, inside a window that
/// keeps the cuts in order and leaves room for the ones after it; with no candidate in
/// range the ideal itself is used. That gives exactly `n` distinct cuts, so the seasons
/// always tile the year — a proposal with a gap or an overlap would be worse than no
/// proposal at all.
pub fn season_cuts(curve: &[f64], n: usize) -> Vec<i64> {
    let length = curve.len() as i64;
    if n == 0 || length == 0 {
        return Vec::new();
    }
    let n = (n as i64).min(length);
    let candidates = turning_points(curve);
    let mut cuts: Vec<i64> = Vec::new();
    for k in 0..n {
        let ideal = k * length / n;
        let lower = cuts.last().map(|c| c + 1).unwrap_or(0);
        let upper = length - (n - k);
        let window = (length / (2 * n)).max(1);
        let pick = candidates
            .iter()
            .copied()
            .filter(|c| *c >= lower && *c <= upper && (*c - ideal).abs() <= window)
            .min_by_key(|c| ((*c - ideal).abs(), *c))
            .unwrap_or_else(|| ideal.clamp(lower, upper));
        cuts.push(pick);
    }
    cuts
}

/// A descriptive placeholder name for a range of the curve.
fn label(curve: &[f64], start: i64, end_exclusive: i64, mean: f64, amplitude: f64) -> &'static str {
    let n = curve.len() as i64;
    if n == 0 {
        return "Season";
    }
    let at = |i: i64| curve[(i.rem_euclid(n)) as usize];
    let net = at(end_exclusive - 1) - at(start);
    let span = (end_exclusive - start).max(1);
    let sum: f64 = (start..end_exclusive).map(at).sum();
    let level = sum / span as f64;
    let threshold = amplitude * 0.1;
    if net > threshold {
        "Rising Light"
    } else if net < -threshold {
        "Falling Light"
    } else if level >= mean {
        "High Sun"
    } else {
        "Low Sun"
    }
}

/// Suggestions creating `n` seasons that tile the year (spec §4.5).
pub fn propose(sky: &SkyModel, n: usize) -> Vec<Suggestion> {
    let cal = sky.calendar();
    let length = cal.month_days();
    if n == 0 || length <= 0 {
        return Vec::new();
    }
    let lat = LatLon::from_degrees(PROPOSAL_LAT_DEG, 0.0);
    let curve: Vec<f64> = (0..length).map(|d| sky.insolation(lat, d as f64)).collect();
    let cuts = season_cuts(&curve, n);
    if cuts.is_empty() {
        return Vec::new();
    }
    let mean = curve.iter().sum::<f64>() / curve.len() as f64;
    let amplitude = {
        let hi = curve.iter().copied().fold(f64::MIN, f64::max);
        let lo = curve.iter().copied().fold(f64::MAX, f64::min);
        (hi - lo).max(0.0)
    };
    let mut used: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut out = Vec::new();
    for (k, start) in cuts.iter().copied().enumerate() {
        // The next cut, wrapped: the last season runs to the day before the first cut.
        let next = if k + 1 < cuts.len() {
            cuts[k + 1]
        } else {
            cuts[0] + length
        };
        let end = next - 1;
        let (start_pos, start_day) = cal.month_day_of(start, false);
        let (end_pos, end_day) = cal.month_day_of(end, false);
        let (Some(start_month), Some(end_month)) = (
            cal.months.get(start_pos).map(|m| m.id),
            cal.months.get(end_pos).map(|m| m.id),
        ) else {
            continue;
        };
        let base = label(&curve, start, next, mean, amplitude);
        let count = used.entry(base).or_insert(0);
        *count += 1;
        let name = if *count == 1 {
            base.to_string()
        } else {
            format!("{base} {count}")
        };
        let mut params = BTreeMap::new();
        params.insert("name".to_string(), Param::Text(name.clone()));
        params.insert("start".to_string(), Param::Int(start));
        params.insert("end".to_string(), Param::Int(end.rem_euclid(length)));
        out.push(Suggestion {
            code: IssueCode::new("astro.add_season"),
            params,
            writes: Vec::new(),
            creates: vec![NewEntity {
                kind: "astro.season".to_string(),
                fields: vec![
                    ("name".to_string(), Value::Text(name)),
                    ("start_month".to_string(), Value::Entity(start_month)),
                    ("start_day".to_string(), Value::Int(start_day)),
                    ("end_month".to_string(), Value::Entity(end_month)),
                    ("end_day".to_string(), Value::Int(end_day)),
                    ("calendar".to_string(), Value::Entity(cal.id)),
                ],
            }],
        });
    }
    out
}
