//! `SkyModel`: the resolved system, and every number the rest of the engine asks of it.

use crate::calendar::CalendarModel;
use crate::error::AstroError;
use crate::kepler::{ecliptic_longitude, mean_anomaly, orbit_position};
use crate::system::{BodyView, HomeSettings, resolve};
use crate::units::BodyClass;
use std::collections::BTreeMap;
use std::f64::consts::PI;
use wb_editlog::{EntityId, State};
use wb_grid::{LatLon, Vec3};

/// A world's whole sky, derived from its entities and never stored.
pub struct SkyModel {
    bodies: Vec<BodyView>,
    index: BTreeMap<EntityId, usize>,
    primary: usize,
    home: usize,
    stars: Vec<usize>,
    moons_of_home: Vec<usize>,
    settings: HomeSettings,
    calendar: CalendarModel,
}

/// Half the arc of daylight, in radians: `acos(−tan φ · tan δ)`.
///
/// Clamping the cosine is what makes polar day (π) and polar night (0) fall out of the
/// same expression instead of needing their own branches.
pub fn hour_angle(lat_rad: f64, declination_rad: f64) -> f64 {
    if !lat_rad.is_finite() || !declination_rad.is_finite() {
        return 0.0;
    }
    let c = -libm::tan(lat_rad) * libm::tan(declination_rad);
    libm::acos(c.clamp(-1.0, 1.0))
}

/// Daily-mean insolation as a fraction of the flux at the sub-stellar point.
///
/// `(1/π)·(H₀·sin φ·sin δ + cos φ·cos δ·sin H₀)` — the standard obliquity geometry,
/// integrated over one rotation.
pub fn daily_mean_factor(lat_rad: f64, declination_rad: f64) -> f64 {
    let h = hour_angle(lat_rad, declination_rad);
    let k = (h * libm::sin(lat_rad) * libm::sin(declination_rad)
        + libm::cos(lat_rad) * libm::cos(declination_rad) * libm::sin(h))
        / PI;
    if k.is_finite() { k.max(0.0) } else { 0.0 }
}

impl SkyModel {
    /// Reads a system from world state.
    ///
    /// Fails only on the six things in [`AstroError`] — a system with no star, two
    /// primaries, a dangling or circular parent, or no calendar. Everything else, however
    /// unstable or absurd, builds fine and is reported by the checkers in §6.
    pub fn build(state: &State) -> Result<SkyModel, AstroError> {
        let settings = HomeSettings::read(state);
        let calendar = CalendarModel::read(state)?;
        let (bodies, primary_id) = resolve(state, &settings)?;
        let index: BTreeMap<EntityId, usize> =
            bodies.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
        let primary = *index.get(&primary_id).ok_or(AstroError::NoPrimaryStar)?;
        let home = *index
            .get(&EntityId::PLANET)
            .expect("the home world is always resolved");
        let stars = bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.is_star())
            .map(|(i, _)| i)
            .collect();
        let moons_of_home = bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.class == BodyClass::Moon && b.parent == Some(EntityId::PLANET))
            .map(|(i, _)| i)
            .collect();
        Ok(SkyModel {
            bodies,
            index,
            primary,
            home,
            stars,
            moons_of_home,
            settings,
            calendar,
        })
    }

    /// Every body, star and the home world, in `EntityId` order.
    pub fn bodies(&self) -> impl Iterator<Item = &BodyView> {
        self.bodies.iter()
    }

    pub fn body(&self, id: EntityId) -> Option<&BodyView> {
        self.index.get(&id).map(|i| &self.bodies[*i])
    }

    pub fn home(&self) -> &BodyView {
        &self.bodies[self.home]
    }

    pub fn primary_star(&self) -> &BodyView {
        &self.bodies[self.primary]
    }

    pub fn stars(&self) -> impl Iterator<Item = &BodyView> {
        self.stars.iter().map(|i| &self.bodies[*i])
    }

    pub fn moons_of_home(&self) -> impl Iterator<Item = &BodyView> {
        self.moons_of_home.iter().map(|i| &self.bodies[*i])
    }

    pub fn calendar(&self) -> &CalendarModel {
        &self.calendar
    }

    pub fn year_days(&self) -> f64 {
        self.settings.year_days
    }

    pub fn day_hours(&self) -> f64 {
        self.settings.day_hours
    }

    pub fn axial_tilt_rad(&self) -> f64 {
        self.settings.axial_tilt_rad
    }

    pub fn is_retrograde(&self) -> bool {
        self.settings.retrograde
    }

    /// Seconds since day 0 — the epoch at which every body is at mean anomaly 0.
    pub fn seconds_of(&self, day: f64) -> f64 {
        day * self.settings.day_s()
    }

    /// Position relative to the primary star, in metres.
    pub fn heliocentric(&self, id: EntityId, day: f64) -> Option<Vec3> {
        let mut i = *self.index.get(&id)?;
        let t = self.seconds_of(day);
        let mut p = Vec3::new(0.0, 0.0, 0.0);
        // Walk up the parent chain, which `resolve` proved is finite.
        for _ in 0..=self.bodies.len() {
            let b = &self.bodies[i];
            let Some(parent) = b.parent else {
                return Some(p);
            };
            p = p.plus(orbit_position(
                b.a_m,
                b.eccentricity,
                b.inclination_rad,
                mean_anomaly(t, b.period_s),
            ));
            i = *self.index.get(&parent)?;
        }
        Some(p)
    }

    /// Position relative to the home world, in metres (spec §4.4).
    pub fn position(&self, id: EntityId, day: f64) -> Option<Vec3> {
        let there = self.heliocentric(id, day)?;
        let here = self.heliocentric(EntityId::PLANET, day)?;
        Some(Vec3::new(
            there.x - here.x,
            there.y - here.y,
            there.z - here.z,
        ))
    }

    /// Declination of the sub-stellar point, radians, for a direction from the home world
    /// to a star.
    ///
    /// `δ = asin(sin ε · sin λ)`, where λ is the star's ecliptic longitude as seen from
    /// the home world. Using the star's own direction rather than the home world's orbital
    /// phase is what makes a second sun's seasons fall out of its orbit.
    pub fn declination(&self, to_star: Vec3) -> f64 {
        let lon = ecliptic_longitude(to_star);
        let s = libm::sin(self.settings.axial_tilt_rad) * libm::sin(lon);
        libm::asin(s.clamp(-1.0, 1.0))
    }

    /// Daily-mean flux at the top of the atmosphere, W·m⁻², summed over every star.
    ///
    /// Stars are summed in `EntityId` order, so the result does not depend on how the
    /// system was assembled. `lat.lon` is ignored: a daily mean is the same all the way
    /// round a parallel.
    pub fn insolation(&self, lat: LatLon, day: f64) -> f64 {
        let mut total = 0.0;
        for i in &self.stars {
            let star = &self.bodies[*i];
            let Some(luminosity) = star.luminosity_w else {
                continue;
            };
            let Some(to_star) = self.position(star.id, day) else {
                continue;
            };
            let distance = to_star.length();
            if distance <= 0.0 || !distance.is_finite() || !luminosity.is_finite() {
                continue;
            }
            let flux = luminosity / (4.0 * PI * distance * distance);
            let k = daily_mean_factor(lat.lat, self.declination(to_star));
            let contribution = flux * k;
            if contribution.is_finite() {
                total += contribution;
            }
        }
        total
    }

    /// Hours of daylight from the primary star, in local hours.
    ///
    /// A companion adds light but not a day: the clock follows the primary (spec §4.2).
    pub fn daylight_hours(&self, lat: LatLon, day: f64) -> f64 {
        let primary = self.primary_star();
        let Some(to_star) = self.position(primary.id, day) else {
            return 0.0;
        };
        let h = hour_angle(lat.lat, self.declination(to_star));
        self.settings.day_hours * h / PI
    }
}
