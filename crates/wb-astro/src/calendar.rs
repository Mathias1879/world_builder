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

/// A date in a world's own calendar. `year` 1 is the epoch year; 0 and below precede it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Date {
    pub year: i64,
    /// The month's own `index` field, not its position.
    pub month_index: i64,
    /// 1-based day of the month.
    pub day: i64,
    pub weekday_index: usize,
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

    /// Days in one month of a given year.
    ///
    /// The leap day goes to `leap_month`; with `leap_rule = every_n` and no month named,
    /// it goes to the last month of the year, so the year length still adds up.
    pub fn days_in(&self, position: usize, leap: bool) -> i64 {
        let Some(m) = self.months.get(position) else {
            return 0;
        };
        let holds_leap_day = match self.leap_month {
            Some(id) => id == m.id,
            None => position + 1 == self.months.len(),
        };
        m.days + i64::from(leap && holds_leap_day)
    }

    /// Whether `year` carries a leap day.
    pub fn is_leap(&self, year: i64) -> bool {
        match self.leap_every_n {
            Some(n) if n >= 2 => year.rem_euclid(n) == 0,
            _ => false,
        }
    }

    /// Days in `year`.
    pub fn year_length(&self, year: i64) -> i64 {
        self.month_days() + i64::from(self.is_leap(year))
    }

    /// Day index of the first day of `year`; `None` on overflow.
    ///
    /// `(Y−1)·base + ⌊(Y−1)/n⌋` with Euclidean division, which is correct on both sides
    /// of the epoch: year 0 is a leap year under every rule, and starts one day earlier
    /// than a bare multiple of the common year would put it.
    pub fn start_of_year(&self, year: i64) -> Option<i64> {
        let years = year.checked_sub(1)?;
        let plain = years.checked_mul(self.month_days())?;
        match self.leap_every_n {
            Some(n) if n >= 2 => plain.checked_add(years.div_euclid(n)),
            _ => Some(plain),
        }
    }

    /// The date on `day_index`.
    pub fn date_of(&self, day_index: i64) -> Date {
        let base = self.month_days().max(1);
        let (year, rest) = match self.leap_every_n {
            Some(n) if n >= 2 => {
                let cycle = n * base + 1;
                let cycles = day_index.div_euclid(cycle);
                let within = day_index.rem_euclid(cycle);
                // Years 1..n−1 of a cycle are common, so plain division finds the year;
                // the leap day only ever extends the last of them.
                let year_in_cycle = (within / base).min(n - 1);
                (
                    cycles * n + year_in_cycle + 1,
                    within - year_in_cycle * base,
                )
            }
            _ => (day_index.div_euclid(base) + 1, day_index.rem_euclid(base)),
        };
        let leap = self.is_leap(year);
        // `rest` is already inside the year here, so `month_day_of`'s leading
        // `rem_euclid` is a no-op; this reuses its month-walking loop rather than
        // repeating it.
        let (position, day) = self.month_day_of(rest, leap);
        let weekday_index = if self.weekdays.is_empty() {
            0
        } else {
            day_index.rem_euclid(self.weekdays.len() as i64) as usize
        };
        Date {
            year,
            month_index: self.months.get(position).map(|m| m.index).unwrap_or(1),
            day,
            weekday_index,
        }
    }

    /// The day index of `date`, or `None` if no such day exists.
    ///
    /// `weekday_index` is derived, so a wrong one is ignored rather than rejected.
    pub fn day_index_of(&self, date: Date) -> Option<i64> {
        let position = self.month_position_by_index(date.month_index)?;
        let leap = self.is_leap(date.year);
        if date.day < 1 || date.day > self.days_in(position, leap) {
            return None;
        }
        let start = self.start_of_year(date.year)?;
        let before: i64 = (0..position).map(|p| self.days_in(p, leap)).sum();
        start.checked_add(before)?.checked_add(date.day - 1)
    }

    /// The first month with this `index`.
    pub fn month_position_by_index(&self, index: i64) -> Option<usize> {
        self.months.iter().position(|m| m.index == index)
    }

    /// Day of year (0-based) for a month position and a 1-based day.
    pub fn day_of_year(&self, position: usize, day: i64, leap: bool) -> i64 {
        let before: i64 = (0..position.min(self.months.len()))
            .map(|p| self.days_in(p, leap))
            .sum();
        before + (day - 1).max(0)
    }

    /// Month position and 1-based day for a 0-based day of year.
    pub fn month_day_of(&self, day_of_year: i64, leap: bool) -> (usize, i64) {
        let length = (self.month_days() + i64::from(leap)).max(1);
        let mut rest = day_of_year.rem_euclid(length);
        let mut position = 0;
        while position + 1 < self.months.len() {
            let days = self.days_in(position, leap);
            if rest < days {
                break;
            }
            rest -= days;
            position += 1;
        }
        (position, rest + 1)
    }

    /// The first named season covering this 0-based day of year, wrap-around included.
    pub fn season_of_day(&self, day_of_year: i64) -> Option<&str> {
        let length = self.month_days().max(1);
        let doy = day_of_year.rem_euclid(length);
        for s in &self.seasons {
            let (Some(start_pos), Some(end_pos)) = (
                self.month_position(s.start_month),
                self.month_position(s.end_month),
            ) else {
                continue;
            };
            let start = self.day_of_year(start_pos, s.start_day, false);
            let end = self.day_of_year(end_pos, s.end_day, false);
            let inside = if start <= end {
                (start..=end).contains(&doy)
            } else {
                // Wraps the year end.
                doy >= start || doy <= end
            };
            if inside {
                return Some(s.name.as_str());
            }
        }
        None
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
