//! `SkyModel`: the resolved system, and every number the rest of the engine asks of it.

use crate::calendar::CalendarModel;
use crate::error::AstroError;
use crate::kepler::{mean_anomaly, orbit_position};
use crate::system::{BodyView, HomeSettings, resolve};
use crate::units::BodyClass;
use std::collections::BTreeMap;
use wb_editlog::{EntityId, State};
use wb_grid::Vec3;

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
}
