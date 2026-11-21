use jiff::{Span, civil::Date};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct DayOfMonth(Date);

impl From<Date> for DayOfMonth {
    fn from(value: Date) -> Self {
        Self(jiff::civil::date(0, value.month(), value.day()))
    }
}

#[macro_export]
macro_rules! day_of_month {
    ($day:expr, $month:expr) => {
        const { DayOfMonth::new_const($day, $month) }
    };
}

impl DayOfMonth {
    #[must_use]
    #[allow(dead_code)]
    pub fn new(day: u8, month: u8) -> Option<Self> {
        let day = i8::try_from(day).ok()?;
        let month = i8::try_from(month).ok()?;
        Date::new(0, month, day).ok().map(Self)
    }

    #[must_use]
    pub const fn new_const(day: u8, month: u8) -> Self {
        Self(jiff::civil::date(0, month.cast_signed(), day.cast_signed()))
    }

    #[must_use]
    pub fn to_date(self, year: i16) -> Option<Date> {
        Date::new(year, self.0.month(), self.0.day()).ok()
    }

    #[must_use]
    pub fn adjust_for_leap_year(mut self, leap_year: bool) -> Self {
        if !leap_year && self.0.month() == 2 && self.0.day() == 29 {
            self.0 = jiff::civil::date(0, 2, 28);
        }
        self
    }

    #[inline]
    #[must_use]
    #[allow(dead_code)]
    pub fn day(self) -> u8 {
        self.0.day().cast_unsigned()
    }

    #[inline]
    #[must_use]
    #[allow(dead_code)]
    pub fn month_num(self) -> u8 {
        self.0.month().cast_unsigned()
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

    pub fn expand_start(mut self, date: Date) -> DateRange {
        let start = self.start.adjust_for_leap_year(date.in_leap_year());
        let Some(start_date) = start.to_date(date.year()) else {
            tracing::warn!("Failed to transform start date: {:?}", self.start);
            return self;
        };
        let start_date = start_date - Span::new().days(1);
        self.start = start_date.into();
        self
    }

    #[must_use]
    pub fn contains(self, other: Date) -> bool {
        let other: DayOfMonth = other.into();
        self.start <= other && other <= self.end
    }

    #[inline]
    #[must_use]
    pub fn should_cache(self, other: Date) -> bool {
        self.expand_start(other).contains(other)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const LEAP_DAY: DayOfMonth = day_of_month!(29, 2);

    #[test]
    fn constructor_rejects_invalid_dates() {
        for (day, month) in [(0, 1), (1, 0), (32, 1), (1, 13), (31, 4), (30, 2)] {
            assert!(DayOfMonth::new(day, month).is_none());
        }
        assert!(DayOfMonth::new(200, 1).is_none());
        assert!(DayOfMonth::new(1, 200).is_none());
    }

    #[test]
    fn const_constructor_accepts_leap_day() {
        assert_eq!(LEAP_DAY, DayOfMonth::new(29, 2).unwrap());
    }

    #[test]
    fn date_conversion_normalizes_year() {
        let leap_day = Date::new(2024, 2, 29).unwrap();
        assert_eq!(DayOfMonth::from(leap_day), LEAP_DAY);
        assert_eq!(DayOfMonth::from(Date::new(2000, 2, 29).unwrap()), LEAP_DAY);
    }

    #[test]
    fn ordering_and_accessors_follow_calendar_date() {
        let january_end = day_of_month!(31, 1);
        let february_start = day_of_month!(1, 2);

        assert!(january_end < february_start);
        assert_eq!(january_end.day(), 31);
        assert_eq!(january_end.month_num(), 1);
    }

    #[test]
    fn leap_day_adjustment_uses_february_28_in_common_years() {
        assert_eq!(LEAP_DAY.adjust_for_leap_year(false), day_of_month!(28, 2));
        assert_eq!(LEAP_DAY.adjust_for_leap_year(true), LEAP_DAY);
    }

    #[test]
    fn invalid_date_for_non_leap_year() {
        let date = day_of_month!(29, 2);
        assert!(date.to_date(2023).is_none());
    }

    #[test]
    fn adjusted_to_valid_date_for_non_leap_year() {
        let date = day_of_month!(29, 2).adjust_for_leap_year(false);
        assert!(date.to_date(2023).is_some());
    }

    #[test]
    fn valid_date_for_leap_year() {
        let date = day_of_month!(29, 2);
        assert!(date.to_date(2024).is_some());
    }

    #[test]
    fn should_not_cache_more_than_one_day_before_start_day() {
        let range = DateRange {
            start: day_of_month!(15, 10),
            end: day_of_month!(31, 10),
        };
        let date = Date::new(2024, 10, 13).unwrap();
        assert!(!range.should_cache(date));
    }

    #[test]
    fn should_cache_one_day_before_start() {
        let range = DateRange {
            start: day_of_month!(15, 10),
            end: day_of_month!(31, 10),
        };
        let date = Date::new(2024, 10, 14).unwrap();
        assert!(range.should_cache(date));
    }

    #[test]
    fn should_cache_ending_day() {
        let range = DateRange {
            start: day_of_month!(15, 10),
            end: day_of_month!(31, 10),
        };
        let date = Date::new(2024, 10, 31).unwrap();
        assert!(range.should_cache(date));
    }

    #[test]
    fn should_not_cache_after_ending_day() {
        let range = DateRange {
            start: day_of_month!(15, 10),
            end: day_of_month!(31, 10),
        };
        let date = Date::new(2024, 11, 1).unwrap();
        assert!(!range.should_cache(date));
    }

    #[test]
    fn date_range_does_not_contain_naive_date_before_start_day() {
        let range = DateRange {
            start: day_of_month!(15, 10),
            end: day_of_month!(31, 10),
        };
        let date = Date::new(2024, 10, 14).unwrap();
        assert!(!range.contains(date));
    }

    #[test]
    fn date_range_does_contain_naive_date_on_start_day() {
        let range = DateRange {
            start: day_of_month!(15, 10),
            end: day_of_month!(31, 10),
        };
        let date = Date::new(2024, 10, 15).unwrap();
        assert!(range.contains(date));
    }

    #[test]
    fn date_range_does_contain_naive_date_on_ending_day() {
        let range = DateRange {
            start: day_of_month!(15, 10),
            end: day_of_month!(31, 10),
        };
        let date = Date::new(2024, 10, 31).unwrap();
        assert!(range.contains(date));
    }

    #[test]
    fn date_range_does_not_contain_naive_date_after_ending_day() {
        let range = DateRange {
            start: day_of_month!(15, 10),
            end: day_of_month!(31, 10),
        };
        let date = Date::new(2024, 11, 1).unwrap();
        assert!(!range.contains(date));
    }
}
