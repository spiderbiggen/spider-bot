use crate::cache;
use crate::commands::gifs::GifError;
use crate::consts::{GIF_COUNT, LONG_CACHE_LIFETIME};
use crate::context::{GifCacheExt, GifContextExt};
use chrono::{Datelike, TimeDelta, Utc};
use chrono::{Month, NaiveDate};
use rand::prelude::SliceRandom;
use rand::{thread_rng, Rng};
use std::collections::HashSet;
use std::num::NonZeroU8;
use std::sync::Arc;
use tenor::Config;
use tracing::{debug, error, info, instrument, warn};
use url::Url;

const SLEEP_GIF_CONFIG: Config = super::RANDOM_CONFIG;

macro_rules! const_nonzero_u8 {
    ($value:expr) => {{
        const RET: NonZeroU8 = {
            let _const_guard: () = [()][($value == 0) as usize];
            // SAFETY: this value is checked at compile time, so it's safe to return it.
            unsafe { NonZeroU8::new_unchecked($value) }
        };
        RET
    }};
}

macro_rules! day_of_month {
    ($day:expr, $month:expr) => {
        DayOfMonth(const_nonzero_u8!($day), $month)
    };
}

static SLEEP_GIF_COLLECTION: &GifCollection = &GifCollection {
    seasons: &[Season {
        range: DateRange {
            start: day_of_month!(15, Month::October),
            end: day_of_month!(31, Month::October),
        },
        resolver: GifResolver {
            name: "halloween sleep",
            ratio_override: Some(RatioQuery {
                query: "https://media.tenor.com/nZm2w7ENZ4AAAAAC/frog-dance.gif",
                numerator: 1,
                denominator: 150,
            }),
            queries: &["halloweensleep", "spookysleep", "horrorsleep"],
        },
    }],
    default: GifResolver {
        name: "sleep",
        ratio_override: Some(RatioQuery {
            query: "https://media.tenor.com/nZm2w7ENZ4AAAAAC/frog-dance.gif",
            numerator: 1,
            denominator: 150,
        }),
        queries: &[
            "sleep",
            "dogsleep",
            "catsleep",
            "rabbitsleep",
            "ratsleep",
            "ducksleep",
            "animalsleep",
        ],
    },
};

#[instrument(skip_all, err)]
pub async fn get_gif(context: &impl GifCacheExt) -> Result<String, GifError> {
    let date = Utc::now().date_naive();
    SLEEP_GIF_COLLECTION
        .current(date)
        .get_gif(context.gif_cache())
        .await
}

pub async fn update_gif_cache(context: &impl GifContextExt<'_>) {
    let date = Utc::now().date_naive();
    for &Season { resolver, range } in SLEEP_GIF_COLLECTION.seasons {
        if !range.should_cache(date) {
            continue;
        }
        if let Err(error) = update_sleep_resolver_cache(context, resolver).await {
            error!("Error caching gifs for {}: {error}", resolver.name);
        }
    }
    let resolver = SLEEP_GIF_COLLECTION.default;
    if let Err(error) = update_sleep_resolver_cache(context, resolver).await {
        error!("Error caching gifs for {}: {error}", resolver.name);
    }
}

#[derive(Debug, Copy, Clone)]
struct DayOfMonth(NonZeroU8, Month);

impl DayOfMonth {
    fn to_naive_date(self, year: i32) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(year, self.1.number_from_month(), u32::from(self.0.get()))
    }

    fn adjust_for_leap_year(mut self, leap_year: bool) -> DayOfMonth {
        if !leap_year && self.1 == Month::February && self.0.get() >= 29 {
            self.0 = const_nonzero_u8!(28);
        }
        self
    }
}

#[derive(Debug, Copy, Clone)]
struct DateRange {
    start: DayOfMonth,
    end: DayOfMonth,
}

impl DateRange {
    fn expand_start(mut self, date: NaiveDate) -> DateRange {
        let start = self.start.adjust_for_leap_year(date.leap_year());
        let Some(mut start_date) = start.to_naive_date(date.year()) else {
            warn!("Failed to transform start date: {:?}", self.start);
            return self;
        };
        start_date -= TimeDelta::days(1);

        let day = u8::try_from(start_date.day()).expect("Chrono days are 1-31");
        let month = u8::try_from(start_date.month()).expect("Chrono month are 1-12");
        self.start = DayOfMonth(
            NonZeroU8::new(day).expect("Chrono days are 1-31"),
            Month::try_from(month).expect("Chrono month are 1-12"),
        );
        self
    }

    fn contains(self, other: NaiveDate) -> bool {
        let day = other.day();
        let month = other.month();
        let start_month = self.start.1.number_from_month();
        let end_month = self.end.1.number_from_month();
        (month >= start_month && month <= end_month)
            && !(month == start_month && day < u32::from(self.start.0.get()))
            && !(month == end_month && day > u32::from(self.end.0.get()))
    }

    fn should_cache(self, other: NaiveDate) -> bool {
        self.expand_start(other).contains(other)
    }
}

#[derive(Debug, Clone, Copy)]
struct GifCollection<'a> {
    seasons: &'a [Season<'a>],
    default: GifResolver<'a>,
}

#[derive(Debug, Clone, Copy)]
struct GifResolver<'a> {
    name: &'static str,
    ratio_override: Option<RatioQuery>,
    queries: CollectionData<'a>,
}

#[derive(Debug, Copy, Clone)]
struct RatioQuery {
    query: &'static str,
    numerator: u32,
    denominator: u32,
}

#[derive(Debug, Clone, Copy)]
struct Season<'a> {
    range: DateRange,
    resolver: GifResolver<'a>,
}

type CollectionData<'a> = &'a [&'a str];

impl<'a> GifCollection<'a> {
    #[must_use]
    #[instrument(skip_all)]
    fn current(&self, date: NaiveDate) -> GifResolver {
        let season = self.seasons.iter().find(|s| s.range.contains(date));
        match season {
            None => self.default,
            Some(season) => {
                debug!("found seasonal {}", season.resolver.name);
                season.resolver
            }
        }
    }
}

impl<'a> GifResolver<'a> {
    #[instrument(skip_all, err)]
    async fn get_gif(&self, gif_cache: &cache::Memory<[Url]>) -> Result<String, GifError> {
        if let Some(query) = self.get_override() {
            debug!("Found gif override");
            return Ok(query.to_string());
        }
        let collection = gif_cache.get(self.name).await.ok_or(GifError::NoGifs)?;
        let gif = collection
            .choose(&mut thread_rng())
            .ok_or(GifError::NoGifs)?;
        Ok(gif.as_str().to_string())
    }

    #[must_use]
    fn get_override(&self) -> Option<&'static str> {
        self.ratio_override
            .filter(|ratio| thread_rng().gen_ratio(ratio.numerator, ratio.denominator))
            .map(|query| query.query)
    }
}

async fn update_sleep_resolver_cache(
    context: &impl GifContextExt<'_>,
    resolver: GifResolver<'_>,
) -> Result<(), GifError> {
    let max_capacity = resolver.queries.len() * usize::from(GIF_COUNT);
    let mut gif_collection: HashSet<Url> = HashSet::with_capacity(max_capacity);
    let (tenor, gif_cache) = context.gif_context();
    for &query in resolver.queries {
        let gifs = tenor.search(query, Some(SLEEP_GIF_CONFIG)).await?;
        gif_collection.extend(gifs.into_iter().map(|gif| gif.url));
    }
    let name = resolver.name;
    let urls: Arc<[Url]> = gif_collection.into_iter().collect();
    let gif_count = urls.len();
    info!(gif_count, "Putting \"{name}\" gifs into cache");
    gif_cache
        .insert_with_duration(name, urls, LONG_CACHE_LIFETIME)
        .await;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn froggers_chance() {
        let mut occurences = 0u32;
        let iterations = 10_000_000u32;
        for _ in 0..iterations {
            if SLEEP_GIF_COLLECTION.default.get_override().is_some() {
                occurences += 1;
            };
        }
        let average_rolls = f64::from(iterations) / f64::from(occurences);
        eprintln!("Froggers average rolls[iterations={iterations}]: {average_rolls:.2}");
        assert!(average_rolls > 149.0 && average_rolls < 151.0);
    }

    #[test]
    fn all_seasons_have_valid_dates() {
        let years = [(2023, false), (2024, true)];
        for (year, leap_year) in years {
            for Season { range, .. } in SLEEP_GIF_COLLECTION.seasons {
                let start = range.start.adjust_for_leap_year(leap_year);
                assert!(
                    start.to_naive_date(year).is_some(),
                    "invalid start date: {year}-{:02}-{:02}",
                    start.1.number_from_month(),
                    start.0.get()
                );
                let end = range.start.adjust_for_leap_year(leap_year);
                assert!(
                    end.to_naive_date(year).is_some(),
                    "invalid start date: {year}-{:02}-{:02}",
                    start.1.number_from_month(),
                    start.0.get()
                );
            }
        }
    }

    #[test]
    fn invalid_date_for_non_leap_year() {
        let date = day_of_month!(29, Month::February);
        assert!(date.to_naive_date(2023).is_none());
    }

    #[test]
    fn adjusted_tovalid_date_for_non_leap_year() {
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
