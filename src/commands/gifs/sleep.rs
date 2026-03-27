use crate::cache::{GifCacheReader, GifCacheWriter};
use crate::commands::gifs::{GifError, get_cached_gif};
use crate::consts::{GIF_COUNT, LONG_CACHE_LIFETIME};
use crate::day_of_month;
use crate::util::{DateRange, DayOfMonth};
use chrono::Utc;
use chrono::{Month, NaiveDate};
use klipy::Klipy;
use klipy::models::Format;
use rand::RngExt;
use rustc_hash::{FxBuildHasher, FxHashSet};
use std::sync::Arc;
use tracing::instrument;
use url::Url;

#[instrument(skip_all, err)]
pub async fn get_gif(gif_cache: &GifCacheReader) -> Result<Arc<Url>, GifError> {
    let date = Utc::now().date_naive();
    SLEEP_GIF_COLLECTION.current(date).get_gif(gif_cache).await
}

#[tracing::instrument(skip_all)]
pub async fn refresh_sleep_gifs(klipy: &Klipy<'_>, writer: &GifCacheWriter) {
    let date = Utc::now().date_naive();
    for Season { resolver, range } in SLEEP_GIF_COLLECTION.seasons {
        if !range.should_cache(date) {
            continue;
        }
        if let Err(error) = refresh_gif_cache_for_resolver(klipy, writer, resolver).await {
            tracing::error!("Error caching gifs for {}: {error}", resolver.name);
        }
    }
    let resolver = &SLEEP_GIF_COLLECTION.default;
    if let Err(error) = refresh_gif_cache_for_resolver(klipy, writer, resolver).await {
        tracing::error!("Error caching gifs for {}: {error}", resolver.name);
        panic!()
    }
}

#[derive(Debug, Clone)]
struct GifCollection<'a> {
    seasons: &'a [Season<'a>],
    default: GifResolver<'a>,
}

#[derive(Debug, Clone)]
struct GifResolver<'a> {
    name: &'static str,
    ratio_override: Option<RatioQuery>,
    queries: CollectionData<'a>,
}

#[derive(Debug, Clone)]
struct RatioQuery {
    gif_url: &'static str,
    numerator: u32,
    denominator: u32,
}

#[derive(Debug, Clone)]
struct Season<'a> {
    range: DateRange,
    resolver: GifResolver<'a>,
}

type CollectionData<'a> = &'a [&'a str];

impl<'gifs> GifCollection<'gifs> {
    #[must_use]
    #[instrument(skip_all)]
    fn current(&self, date: NaiveDate) -> &GifResolver<'gifs> {
        let season = self.seasons.iter().find(|s| s.range.contains(date));
        match season {
            None => &self.default,
            Some(season) => {
                tracing::debug!("found seasonal {}", season.resolver.name);
                &season.resolver
            }
        }
    }
}

impl GifResolver<'_> {
    #[instrument(skip_all, err)]
    async fn get_gif(&self, gif_cache: &GifCacheReader) -> Result<Arc<Url>, GifError> {
        if let Some(query) = self.get_override(&mut rand::rng()) {
            tracing::debug!("Found gif override");
            match query.parse() {
                Ok(url) => return Ok(Arc::new(url)),
                Err(error) => tracing::warn!("Error parsing gif override: {error}"),
            }
        }
        get_cached_gif(gif_cache, self.name)
    }

    #[must_use]
    fn get_override(&self, rng: &mut impl rand::Rng) -> Option<&'static str> {
        self.ratio_override
            .as_ref()
            .filter(|ratio| rng.random_ratio(ratio.numerator, ratio.denominator))
            .map(|query| query.gif_url)
    }
}

async fn refresh_gif_cache_for_resolver(
    klipy: &Klipy<'_>,
    writer: &GifCacheWriter,
    resolver: &GifResolver<'_>,
) -> Result<(), GifError> {
    let max_capacity = resolver.queries.len() * usize::from(GIF_COUNT);
    let mut gif_collection: FxHashSet<Url> =
        FxHashSet::with_capacity_and_hasher(max_capacity, FxBuildHasher);

    for &query in resolver.queries {
        let gifs = klipy.search(query, None).await?;
        gif_collection.extend(
            gifs.into_iter()
                .filter_map(|gif| gif.into_media(Format::Gif)),
        );
    }
    let name = resolver.name;
    let urls: Box<[Arc<Url>]> = gif_collection.into_iter().map(Arc::new).collect();
    let gif_count = urls.len();
    if writer.insert_with_duration(name, urls, LONG_CACHE_LIFETIME) {
        tracing::info!(gif_count, "Put \"{name}\" gifs into cache");
    }
    Ok(())
}

const FROGGERS_RATIO_QUERY: RatioQuery = RatioQuery {
    gif_url: "https://klipy.com/gifs/sexy-frog",
    numerator: 1,
    denominator: 150,
};

static SLEEP_GIF_COLLECTION: &GifCollection = &GifCollection {
    seasons: &[
        Season {
            range: DateRange::new(
                day_of_month!(15, Month::October),
                day_of_month!(31, Month::October),
            ),
            resolver: GifResolver {
                name: "halloween sleep",
                ratio_override: Some(FROGGERS_RATIO_QUERY),
                queries: &["halloween_sleep", "spooky_sleep", "horror_sleep"],
            },
        },
        Season {
            range: DateRange::new(
                day_of_month!(2, Month::April),
                day_of_month!(6, Month::April),
            ),
            resolver: GifResolver {
                name: "easter sleep",
                ratio_override: Some(FROGGERS_RATIO_QUERY),
                queries: &["easter bunny sleep", "egg sleep", "easter", "easter island"],
            },
        },
    ],
    default: GifResolver {
        name: "sleep",
        ratio_override: Some(FROGGERS_RATIO_QUERY),
        queries: &[
            "sleep",
            "dog Sleep",
            "cat sleep",
            "rabbit sleep",
            "rat sleep",
            "duck sleep",
            "sheep sleep",
            "animal sleep",
            "kirby sleep",
        ],
    },
};

#[cfg(test)]
mod test {
    use super::*;
    use rand::rand_core::{Infallible, TryRng};

    /// A deterministic RNG that always returns the same `u64` value.
    /// `Bernoulli::sample` calls `rng.random()` → `next_u64()` and returns
    /// `true` iff the value is less than `p_int`.  So:
    ///   - `ConstRng(0)`         → always triggers (0 < any `p_int` > 0)
    ///   - `ConstRng(u64::MAX)`  → never triggers  (MAX ≥ any `p_int` < MAX)
    struct ConstRng(u64);

    impl TryRng for ConstRng {
        type Error = Infallible;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            // Tests only exercise try_next_u64; truncate to lower 32 bits as a
            // reasonable fallback (0 stays 0, u64::MAX wraps to u32::MAX).
            Ok(u32::try_from(self.0).unwrap_or(u32::MAX))
        }
        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Ok(self.0)
        }
        fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
            let bytes = self.0.to_le_bytes();
            for (i, b) in dst.iter_mut().enumerate() {
                *b = bytes[i % 8];
            }
            Ok(())
        }
    }

    #[test]
    fn all_resolvers_have_froggers_ratio_of_one_in_150() {
        let all_resolvers = SLEEP_GIF_COLLECTION
            .seasons
            .iter()
            .map(|s| &s.resolver)
            .chain(std::iter::once(&SLEEP_GIF_COLLECTION.default));

        for resolver in all_resolvers {
            let ratio = resolver
                .ratio_override
                .as_ref()
                .unwrap_or_else(|| panic!("resolver \"{}\" has no ratio_override", resolver.name));
            assert_eq!(
                ratio.numerator, 1,
                "resolver \"{}\" has wrong numerator",
                resolver.name
            );
            assert_eq!(
                ratio.denominator, 150,
                "resolver \"{}\" has wrong denominator",
                resolver.name
            );
        }
    }

    #[test]
    fn get_override_returns_url_when_ratio_fires() {
        let resolver = &SLEEP_GIF_COLLECTION.default;
        let result = resolver.get_override(&mut ConstRng(0));
        assert_eq!(result, Some(FROGGERS_RATIO_QUERY.gif_url));
    }

    #[test]
    fn get_override_returns_none_when_ratio_does_not_fire() {
        let resolver = &SLEEP_GIF_COLLECTION.default;
        let result = resolver.get_override(&mut ConstRng(u64::MAX));
        assert_eq!(result, None);
    }

    #[test]
    fn all_seasons_have_valid_dates() {
        let years = [(2023, false), (2024, true), (2025, false)];
        for (year, leap_year) in years {
            for Season { range, .. } in SLEEP_GIF_COLLECTION.seasons {
                let start = range.start.adjust_for_leap_year(leap_year);
                assert!(
                    start.to_naive_date(year).is_some(),
                    "invalid start date: {year}-{:02}-{:02}",
                    start.month_num(),
                    start.day()
                );
                let end = range.end.adjust_for_leap_year(leap_year);
                assert!(
                    end.to_naive_date(year).is_some(),
                    "invalid end date: {year}-{:02}-{:02}",
                    end.month_num(),
                    end.day()
                );
            }
        }
    }
}
