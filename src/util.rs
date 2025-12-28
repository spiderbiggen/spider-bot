use chrono::{Datelike, TimeDelta};
use chrono::{Month, NaiveDate};
use std::cmp::Ordering;
use std::num::{NonZero, NonZeroU8};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct DayOfMonth(NonZeroU8, Month);

impl Ord for DayOfMonth {
    fn cmp(&self, other: &Self) -> Ordering {
        self.1.cmp(&other.1).then(self.0.cmp(&other.0))
    }
}

impl PartialOrd for DayOfMonth {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<NaiveDate> for DayOfMonth {
    fn from(value: NaiveDate) -> Self {
        let day = u8::try_from(value.day())
            .ok()
            .and_then(NonZeroU8::new)
            .expect("Chrono days are 1-31");
        let month = u8::try_from(value.month()).expect("Chrono month is 1-12");
        DayOfMonth(day, Month::try_from(month).expect("Chrono month is 1-12"))
    }
}

#[macro_export]
macro_rules! day_of_month {
    ($day:expr, $month:expr) => {
        const { DayOfMonth::new($day, $month).expect("day cannot be zero") }
    };
}

impl DayOfMonth {
    #[must_use]
    pub const fn new(day: u8, month: Month) -> Option<Self> {
        match NonZeroU8::new(day) {
            Some(day) => Some(DayOfMonth(day, month)),
            None => None,
        }
    }

    #[must_use]
    pub fn to_naive_date(self, year: i32) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(year, self.1.number_from_month(), u32::from(self.0.get()))
    }

    #[must_use]
    pub fn adjust_for_leap_year(mut self, leap_year: bool) -> DayOfMonth {
        if !leap_year && self.1 == Month::February && self.0.get() >= 29 {
            self.0 = const { NonZero::new(28).unwrap() };
        }
        self
    }

    #[inline]
    #[must_use]
    #[allow(dead_code)]
    pub fn day(self) -> u8 {
        self.0.get()
    }

    #[inline]
    #[must_use]
    #[allow(dead_code)]
    pub fn month_num(self) -> u32 {
        self.1.number_from_month()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct DateRange {
    pub start: DayOfMonth,
    pub end: DayOfMonth,
}

impl DateRange {
    pub const fn new(start: DayOfMonth, end: DayOfMonth) -> Self {
        Self { start, end }
    }

    pub fn expand_start(mut self, date: NaiveDate) -> DateRange {
        let start = self.start.adjust_for_leap_year(date.leap_year());
        let Some(mut start_date) = start.to_naive_date(date.year()) else {
            tracing::warn!("Failed to transform start date: {:?}", self.start);
            return self;
        };
        start_date -= TimeDelta::days(1);
        self.start = start_date.into();
        self
    }

    #[must_use]
    pub fn contains(self, other: NaiveDate) -> bool {
        let other: DayOfMonth = other.into();
        self.start <= other && other <= self.end
    }

    #[inline]
    #[must_use]
    pub fn should_cache(self, other: NaiveDate) -> bool {
        self.expand_start(other).contains(other)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn invalid_date_for_non_leap_year() {
        let date = day_of_month!(29, Month::February);
        assert!(date.to_naive_date(2023).is_none());
    }

    #[test]
    fn adjusted_to_valid_date_for_non_leap_year() {
        let date = day_of_month!(29, Month::February).adjust_for_leap_year(false);
        assert!(date.to_naive_date(2023).is_some());
    }

    #[test]
    fn valid_date_for_leap_year() {
        let date = day_of_month!(29, Month::February);
        assert!(date.to_naive_date(2024).is_some());
    }

    #[test]
    fn should_not_cache_more_than_one_day_before_start_day() {
        let range = DateRange {
            start: day_of_month!(15, Month::October),
            end: day_of_month!(31, Month::October),
        };
        let date = NaiveDate::from_ymd_opt(2024, 10, 13).unwrap();
        assert!(!range.should_cache(date));
    }

    #[test]
    fn should_cache_one_day_before_start() {
        let range = DateRange {
            start: day_of_month!(15, Month::October),
            end: day_of_month!(31, Month::October),
        };
        let date = NaiveDate::from_ymd_opt(2024, 10, 14).unwrap();
        assert!(range.should_cache(date));
    }

    #[test]
    fn should_cache_ending_day() {
        let range = DateRange {
            start: day_of_month!(15, Month::October),
            end: day_of_month!(31, Month::October),
        };
        let date = NaiveDate::from_ymd_opt(2024, 10, 31).unwrap();
        assert!(range.should_cache(date));
    }

    #[test]
    fn should_not_cache_after_ending_day() {
        let range = DateRange {
            start: day_of_month!(15, Month::October),
            end: day_of_month!(31, Month::October),
        };
        let date = NaiveDate::from_ymd_opt(2024, 11, 1).unwrap();
        assert!(!range.should_cache(date));
    }

    #[test]
    fn date_range_does_not_contain_naive_date_before_start_day() {
        let range = DateRange {
            start: day_of_month!(15, Month::October),
            end: day_of_month!(31, Month::October),
        };
        let date = NaiveDate::from_ymd_opt(2024, 10, 14).unwrap();
        assert!(!range.contains(date));
    }

    #[test]
    fn date_range_does_contain_naive_date_on_start_day() {
        let range = DateRange {
            start: day_of_month!(15, Month::October),
            end: day_of_month!(31, Month::October),
        };
        let date = NaiveDate::from_ymd_opt(2024, 10, 15).unwrap();
        assert!(range.contains(date));
    }

    #[test]
    fn date_range_does_contain_naive_date_on_ending_day() {
        let range = DateRange {
            start: day_of_month!(15, Month::October),
            end: day_of_month!(31, Month::October),
        };
        let date = NaiveDate::from_ymd_opt(2024, 10, 31).unwrap();
        assert!(range.contains(date));
    }

    #[test]
    fn date_range_does_not_contain_naive_date_after_ending_day() {
        let range = DateRange {
            start: day_of_month!(15, Month::October),
            end: day_of_month!(31, Month::October),
        };
        let date = NaiveDate::from_ymd_opt(2024, 11, 1).unwrap();
        assert!(!range.contains(date));
    }
}
