//! The calendar as data. Date arithmetic lives alongside it; this is what is read from
//! the entities.

use crate::error::AstroError;
use crate::kinds::{int, words};
use wb_editlog::{EntityId, State, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct MonthView {
    pub id: EntityId,
    pub index: i64,
    pub name: String,
    pub days: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SeasonView {
    pub id: EntityId,
    pub name: String,
    pub start_month: EntityId,
    pub start_day: i64,
    pub end_month: EntityId,
    pub end_day: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalendarModel {
    pub id: EntityId,
    pub epoch_name: String,
    pub weekdays: Vec<String>,
    /// `Some(n)` when the leap rule is `every_n`.
    pub leap_every_n: Option<i64>,
    pub leap_month: Option<EntityId>,
    /// Months in `(index, EntityId)` order.
    pub months: Vec<MonthView>,
    /// Seasons in `EntityId` order.
    pub seasons: Vec<SeasonView>,
}

fn text(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::Text(s)) => Some(s.clone()),
        _ => None,
    }
}

fn entity(v: Option<&Value>) -> Option<EntityId> {
    match v {
        Some(Value::Entity(e)) => Some(*e),
        _ => None,
    }
}

impl CalendarModel {
    /// The stated month days of a common (non-leap) year.
    pub fn month_days(&self) -> i64 {
        self.months.iter().map(|m| m.days).sum()
    }

    pub fn month(&self, id: EntityId) -> Option<&MonthView> {
        self.months.iter().find(|m| m.id == id)
    }

    /// Position of a month in `months`, which is its place in the year.
    pub fn month_position(&self, id: EntityId) -> Option<usize> {
        self.months.iter().position(|m| m.id == id)
    }

    /// Days in one month of a given year, honouring the leap rule.
    pub fn days_in(&self, position: usize, leap: bool) -> i64 {
        match self.months.get(position) {
            None => 0,
            Some(m) => {
                let bonus = i64::from(leap && self.leap_month == Some(m.id));
                m.days + bonus
            }
        }
    }

    /// Reads the one calendar and its months and seasons.
    ///
    /// With several `astro.calendar` entities the lowest `EntityId` wins — the one made
    /// first. Months and seasons naming a different calendar are skipped.
    pub(crate) fn read(state: &State) -> Result<CalendarModel, AstroError> {
        let cal = state
            .live()
            .find(|e| e.kind() == Some("astro.calendar"))
            .ok_or(AstroError::NoCalendar)?;
        let id = cal.id;
        let mut months: Vec<MonthView> = Vec::new();
        let mut seasons: Vec<SeasonView> = Vec::new();
        for e in state.live() {
            match e.kind() {
                Some("astro.month") if entity(e.get("calendar")) == Some(id) => {
                    let (Some(index), Some(days), Some(name)) =
                        (int(&e, "index"), int(&e, "days"), text(e.get("name")))
                    else {
                        continue;
                    };
                    if days >= 1 && index >= 1 {
                        months.push(MonthView {
                            id: e.id,
                            index,
                            name,
                            days,
                        });
                    }
                }
                Some("astro.season") if entity(e.get("calendar")) == Some(id) => {
                    let (
                        Some(name),
                        Some(start_month),
                        Some(start_day),
                        Some(end_month),
                        Some(end_day),
                    ) = (
                        text(e.get("name")),
                        entity(e.get("start_month")),
                        int(&e, "start_day"),
                        entity(e.get("end_month")),
                        int(&e, "end_day"),
                    )
                    else {
                        continue;
                    };
                    seasons.push(SeasonView {
                        id: e.id,
                        name,
                        start_month,
                        start_day,
                        end_month,
                        end_day,
                    });
                }
                _ => {}
            }
        }
        if months.is_empty() {
            return Err(AstroError::NoMonths);
        }
        months.sort_by_key(|m| (m.index, m.id));
        seasons.sort_by_key(|s| s.id);
        let leap_every_n = match text(cal.get("leap_rule")).as_deref() {
            Some("every_n") => int(&cal, "leap_every_n").filter(|n| *n >= 2),
            _ => None,
        };
        Ok(CalendarModel {
            id,
            epoch_name: text(cal.get("epoch_name")).unwrap_or_default(),
            weekdays: words(&cal, "weekdays")
                .unwrap_or_default()
                .into_iter()
                .map(str::to_string)
                .collect(),
            leap_every_n,
            leap_month: entity(cal.get("leap_month")).filter(|m| months.iter().any(|x| x.id == *m)),
            months,
            seasons,
        })
    }
}
