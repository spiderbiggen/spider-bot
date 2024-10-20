use crate::cache;
use crate::commands::gifs::{GifError, BASE_GIF_CONFIG, GIF_COUNT};
use chrono::{Datelike, Utc};
use chrono::{Month, NaiveDate};
use rand::prelude::SliceRandom;
use rand::{thread_rng, Rng};
use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;
use tenor::Config;
use tracing::{debug, error, info, instrument};
use url::Url;

const SLEEP_GIF_CONFIG: Config = BASE_GIF_CONFIG.random(true);

static SLEEP_GIF_COLLECTION: &GifCollection = &GifCollection {
    seasons: &[Season {
        range: DateRange {
            start: DayOfMonth(15, Month::October),
            end: DayOfMonth(31, Month::October),
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
pub async fn get_gif(gif_cache: &cache::Memory<[Url]>) -> Result<Cow<'static, str>, GifError> {
    let date = Utc::now().date_naive();
    SLEEP_GIF_COLLECTION.current(date).get_gif(gif_cache).await
}

pub async fn update_gif_cache(tenor: &tenor::Client<'_>, gif_cache: &cache::Memory<[Url]>) {
    for &Season { resolver, .. } in SLEEP_GIF_COLLECTION.seasons {
        if let Err(error) = update_sleep_resolver_cache(tenor, gif_cache, resolver).await {
            error!("Error caching gifs for {}: {error}", resolver.name);
        }
    }
    let resolver = SLEEP_GIF_COLLECTION.default;
    if let Err(error) = update_sleep_resolver_cache(tenor, gif_cache, resolver).await {
        error!("Error caching gifs for {}: {error}", resolver.name);
    }
}

#[derive(Debug, Copy, Clone)]
struct DayOfMonth(u8, Month);

#[derive(Debug, Copy, Clone)]
struct DateRange {
    start: DayOfMonth,
    end: DayOfMonth,
}

impl DateRange {
    fn contains(self, other: NaiveDate) -> bool {
        let day = other.day();
        let month = other.month();
        let start_month = self.start.1.number_from_month();
        let end_month = self.end.1.number_from_month();
        (month >= start_month && month <= end_month)
            && !(month == start_month && day < u32::from(self.start.0))
            && !(month == end_month && day > u32::from(self.end.0))
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
    async fn get_gif(
        &self,
        gif_cache: &cache::Memory<[Url]>,
    ) -> Result<Cow<'static, str>, GifError> {
        if let Some(query) = self.get_override() {
            debug!("Found gif override");
            return Ok(Cow::Borrowed(query));
        }
        let collection = gif_cache.get(self.name).await.ok_or(GifError::NoGifs)?;
        let gif = collection
            .choose(&mut thread_rng())
            .ok_or(GifError::NoGifs)?;
        Ok(gif.as_str().to_string().into())
    }

    #[must_use]
    fn get_override(&self) -> Option<&'static str> {
        self.ratio_override
            .filter(|ratio| thread_rng().gen_ratio(ratio.numerator, ratio.denominator))
            .map(|query| query.query)
    }
}

async fn update_sleep_resolver_cache(
    tenor: &tenor::Client<'_>,
    gif_cache: &cache::Memory<[Url]>,
    resolver: GifResolver<'_>,
) -> Result<(), GifError> {
    let max_capacity = resolver.queries.len() * usize::from(GIF_COUNT);
    let mut gif_collection: HashSet<Url> = HashSet::with_capacity(max_capacity);
    for &query in resolver.queries {
        let gifs = tenor.search(query, Some(SLEEP_GIF_CONFIG)).await?;
        gif_collection.extend(gifs.into_iter().map(|gif| gif.url));
    }
    let name = resolver.name;
    let urls: Arc<[Url]> = gif_collection.into_iter().collect();
    let gif_count = urls.len();
    info!(gif_count, "Putting \"{name}\" gifs into cache");
    gif_cache.insert(name, urls).await;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn froggers_chance() {
        let mut sum = 0u32;
        let iterations = 100_000u32;
        (0..iterations).for_each(|_| {
            let mut counter = 1;
            loop {
                if SLEEP_GIF_COLLECTION.default.get_override()
                    == Some("https://media.tenor.com/nZm2w7ENZ4AAAAAC/frog-dance.gif")
                {
                    break;
                }
                counter += 1;
            }
            sum += counter;
        });
        let average_rolls = f64::from(sum) / f64::from(iterations);
        eprintln!("Froggers average rolls[iterations={iterations}]: {average_rolls:.2}");
        assert!(average_rolls > 149.0 && average_rolls < 151.0);
    }
}
