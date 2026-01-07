use crate::Tenor;
use crate::commands::gifs::{GifError, get_cached_gif};
use crate::consts::{GIF_COUNT, LONG_CACHE_LIFETIME};
use crate::util::{DateRange, DayOfMonth};
use crate::{GifCache, day_of_month};
use chrono::Utc;
use chrono::{Month, NaiveDate};
use rand::Rng;
use rustc_hash::{FxBuildHasher, FxHashSet};
use std::sync::Arc;
use tenor::Config;
use tracing::instrument;
use url::Url;

const SLEEP_GIF_CONFIG: Config = super::RANDOM_CONFIG;

#[instrument(skip_all, err)]
pub async fn get_gif(gif_cache: &GifCache) -> Result<Arc<Url>, GifError> {
    let date = Utc::now().date_naive();
    SLEEP_GIF_COLLECTION.current(date).get_gif(gif_cache).await
}

pub async fn refresh_gif_cache(tenor: &Tenor<'_>, gif_cache: &GifCache) {
    let date = Utc::now().date_naive();
    for Season { resolver, range } in SLEEP_GIF_COLLECTION.seasons {
        if !range.should_cache(date) {
            continue;
        }
        if let Err(error) = refresh_gif_cache_for_resolver(tenor, gif_cache, resolver).await {
            tracing::error!("Error caching gifs for {}: {error}", resolver.name);
        }
    }
    let resolver = &SLEEP_GIF_COLLECTION.default;
    if let Err(error) = refresh_gif_cache_for_resolver(tenor, gif_cache, resolver).await {
        tracing::error!("Error caching gifs for {}: {error}", resolver.name);
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
    async fn get_gif(&self, gif_cache: &GifCache) -> Result<Arc<Url>, GifError> {
        if let Some(query) = self.get_override() {
            tracing::debug!("Found gif override");
            match query.parse() {
                Ok(url) => return Ok(Arc::new(url)),
                Err(error) => tracing::warn!("Error parsing gif override: {error}"),
            }
        }
        get_cached_gif(gif_cache, self.name)
    }

    #[must_use]
    fn get_override(&self) -> Option<&'static str> {
        self.ratio_override
            .as_ref()
            .filter(|ratio| rand::rng().random_ratio(ratio.numerator, ratio.denominator))
            .map(|query| query.gif_url)
    }
}

async fn refresh_gif_cache_for_resolver(
    tenor: &Tenor<'_>,
    gif_cache: &GifCache,
    resolver: &GifResolver<'_>,
) -> Result<(), GifError> {
    let max_capacity = resolver.queries.len() * usize::from(GIF_COUNT);
    let mut gif_collection: FxHashSet<Url> =
        FxHashSet::with_capacity_and_hasher(max_capacity, FxBuildHasher);

    for &query in resolver.queries {
        let gifs = tenor.search(query, Some(SLEEP_GIF_CONFIG)).await?;
        gif_collection.extend(gifs.into_iter().map(|gif| gif.url));
    }
    let name = resolver.name;
    let urls: Box<[Arc<Url>]> = gif_collection.into_iter().map(Arc::new).collect();
    let gif_count = urls.len();
    if gif_cache.insert_with_duration(name, urls, LONG_CACHE_LIFETIME) {
        tracing::info!(gif_count, "Put \"{name}\" gifs into cache");
    }
    Ok(())
}

const FROGGERS_RATIO_QUERY: RatioQuery = RatioQuery {
    gif_url: "https://media.tenor.com/nZm2w7ENZ4AAAAAC/frog-dance.gif",
    numerator: 1,
    denominator: 150,
};

static SLEEP_GIF_COLLECTION: &GifCollection = &GifCollection {
    seasons: &[Season {
        range: DateRange::new(
            day_of_month!(15, Month::October),
            day_of_month!(31, Month::October),
        ),
        resolver: GifResolver {
            name: "halloween sleep",
            ratio_override: Some(FROGGERS_RATIO_QUERY),
            queries: &["halloween_sleep", "spooky_sleep", "horror_sleep"],
        },
    }],
    default: GifResolver {
        name: "sleep",
        ratio_override: Some(FROGGERS_RATIO_QUERY),
        queries: &[
            "sleep",
            "dog_sleep",
            "cat_sleep",
            "rabbit_sleep",
            "rat_sleep",
            "duck_sleep",
            "sheep_sleep",
            "animal_sleep",
        ],
    },
};

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
            }
        }
        let average_rolls = f64::from(iterations) / f64::from(occurences);
        eprintln!("Froggers average rolls[iterations={iterations}]: {average_rolls:.2}");
        assert!(average_rolls > 149.0 && average_rolls < 151.0);
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
