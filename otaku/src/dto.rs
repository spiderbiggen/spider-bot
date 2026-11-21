//! Wire format of the anime service's `download` server-sent events.

use domain::DownloadVariant;
use serde::{Deserialize, Deserializer};
use std::num::TryFromIntError;
use url::Url;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    TryFromIntError(#[from] TryFromIntError),
}

#[derive(Debug, Deserialize)]
pub struct DownloadCollection {
    pub title: String,
    #[serde(flatten)]
    pub variant: Variant,
    pub created_at: jiff::Timestamp,
    pub updated_at: jiff::Timestamp,
    pub downloads: Vec<Download>,
    /// Show metadata matched by the service. Purely cosmetic, so a malformed value is dropped
    /// instead of failing the whole message.
    #[serde(default, deserialize_with = "lenient_show")]
    pub show: Option<Show>,
}

#[derive(Debug, Deserialize)]
pub struct Show {
    pub tmdb_id: u32,
    #[serde(default)]
    pub tvdb_id: Option<u32>,
    pub name: String,
    #[serde(default)]
    pub romaji_name: Option<String>,
    #[serde(default)]
    pub original_name: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub poster_url: Option<String>,
    #[serde(default)]
    pub backdrop_url: Option<String>,
}

fn lenient_show<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Show>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| match serde_json::from_value(value) {
        Ok(show) => Some(show),
        Err(err) => {
            tracing::warn!("Ignoring malformed show metadata: {err}");
            None
        }
    }))
}

#[derive(Debug, Deserialize)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum Variant {
    Episode {
        episode: u32,
        #[serde(default)]
        decimal: Option<u32>,
        #[serde(default)]
        version: Option<u32>,
        #[serde(default)]
        extra: Option<String>,
    },
    Batch {
        start: u32,
        end: u32,
    },
    Movie,
}

#[derive(Debug, Deserialize)]
pub struct Download {
    pub comments: String,
    pub resolution: u32,
    pub torrent: String,
    pub file_name: String,
    pub published_date: jiff::Timestamp,
}

impl TryFrom<DownloadCollection> for domain::DownloadCollection {
    type Error = Error;

    fn try_from(value: DownloadCollection) -> Result<Self, Self::Error> {
        Ok(Self {
            title: value.title,
            variant: value.variant.into(),
            created_at: value.created_at,
            updated_at: value.updated_at,
            downloads: value
                .downloads
                .into_iter()
                .map(domain::Download::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            show: value.show.map(Into::into),
        })
    }
}

impl From<Show> for domain::Show {
    fn from(value: Show) -> Self {
        Self {
            tmdb_id: value.tmdb_id,
            tvdb_id: value.tvdb_id,
            name: value.name,
            romaji_name: non_empty(value.romaji_name),
            original_name: non_empty(value.original_name),
            overview: non_empty(value.overview),
            poster_url: valid_url("poster_url", value.poster_url),
            backdrop_url: valid_url("backdrop_url", value.backdrop_url),
        }
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

/// Discord rejects an embed with an invalid image URL, so only pass on http(s) URLs.
fn valid_url(field: &str, value: Option<String>) -> Option<String> {
    let value = non_empty(value)?;
    match Url::parse(&value) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => Some(value),
        _ => {
            tracing::warn!("Ignoring invalid show {field}: {value:?}");
            None
        }
    }
}

impl From<Variant> for DownloadVariant {
    fn from(value: Variant) -> Self {
        match value {
            Variant::Episode {
                episode,
                decimal,
                version,
                extra,
            } => DownloadVariant::Episode(domain::Episode {
                number: episode,
                decimal: decimal.filter(|&d| d != 0),
                version: version.filter(|&v| v != 0),
                extra: extra.filter(|s| !s.is_empty()),
            }),
            Variant::Batch { start, end } => DownloadVariant::Batch(start..=end),
            Variant::Movie => DownloadVariant::Movie,
        }
    }
}

impl TryFrom<Download> for domain::Download {
    type Error = Error;

    fn try_from(value: Download) -> Result<Self, Self::Error> {
        Ok(Self {
            published_date: value.published_date,
            resolution: u16::try_from(value.resolution)?,
            comments: value.comments,
            torrent: value.torrent,
            file_name: value.file_name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOWNLOADS: &str = r#"[{"comments":"https://nyaa.si/view/1","resolution":1080,
        "torrent":"https://nyaa.si/download/1.torrent","file_name":"[SubsPlease] Show - 07 (1080p).mkv",
        "published_date":"2026-11-20T16:02:28Z"}]"#;

    fn parse(json: &str) -> domain::DownloadCollection {
        let dto: DownloadCollection = serde_json::from_str(json).unwrap();
        dto.try_into().unwrap()
    }

    fn ts(s: &str) -> jiff::Timestamp {
        s.parse().unwrap()
    }

    fn expected_downloads() -> Vec<domain::Download> {
        vec![domain::Download {
            published_date: ts("2026-11-20T16:02:28Z"),
            resolution: 1080,
            comments: "https://nyaa.si/view/1".into(),
            torrent: "https://nyaa.si/download/1.torrent".into(),
            file_name: "[SubsPlease] Show - 07 (1080p).mkv".into(),
        }]
    }

    #[test]
    fn episode_with_all_fields() {
        let json = format!(
            r#"{{"title":"Show","variant":"episode","episode":7,"decimal":5,"version":2,"extra":"END",
            "created_at":"2026-11-20T16:02:24Z","updated_at":"2026-11-20T16:02:28Z","downloads":{DOWNLOADS}}}"#
        );
        assert_eq!(
            parse(&json),
            domain::DownloadCollection {
                title: "Show".into(),
                variant: DownloadVariant::Episode(domain::Episode {
                    number: 7,
                    decimal: Some(5),
                    version: Some(2),
                    extra: Some("END".into()),
                }),
                created_at: ts("2026-11-20T16:02:24Z"),
                updated_at: ts("2026-11-20T16:02:28Z"),
                downloads: expected_downloads(),
                show: None,
            }
        );
    }

    #[test]
    fn episode_with_optional_fields_missing_null_or_zero() {
        let json = format!(
            r#"{{"title":"Show","variant":"episode","episode":7,"decimal":0,"version":null,"extra":"",
            "created_at":"2026-11-20T16:02:24Z","updated_at":"2026-11-20T16:02:28Z","downloads":{DOWNLOADS}}}"#
        );
        assert_eq!(
            parse(&json).variant,
            DownloadVariant::Episode(domain::Episode {
                number: 7,
                decimal: None,
                version: None,
                extra: None,
            })
        );
    }

    #[test]
    fn batch() {
        let json = format!(
            r#"{{"title":"Show","variant":"batch","start":1,"end":12,
            "created_at":"2026-11-20T16:02:24Z","updated_at":"2026-11-20T16:02:28Z","downloads":{DOWNLOADS}}}"#
        );
        assert_eq!(parse(&json).variant, DownloadVariant::Batch(1..=12));
    }

    #[test]
    fn movie() {
        let json = format!(
            r#"{{"title":"Show","variant":"movie",
            "created_at":"2026-11-20T16:02:24Z","updated_at":"2026-11-20T16:02:28Z","downloads":{DOWNLOADS}}}"#
        );
        let collection = parse(&json);
        assert_eq!(collection.variant, DownloadVariant::Movie);
        assert_eq!(collection.downloads, expected_downloads());
    }

    #[test]
    fn unknown_fields_and_show_are_accepted() {
        let json = format!(
            r#"{{"title":"Show","variant":"movie","brand_new":{{"a":1}},
            "created_at":"2026-11-20T16:02:24Z","updated_at":"2026-11-20T16:02:28Z","downloads":{DOWNLOADS},
            "show":{{"tmdb_id":1,"tvdb_id":2,"name":"Show","overview":"...","poster_url":"https://p","backdrop_url":"https://b"}}}}"#
        );
        assert_eq!(parse(&json).variant, DownloadVariant::Movie);

        let minimal_show = format!(
            r#"{{"title":"Show","variant":"movie","created_at":"2026-11-20T16:02:24Z",
            "updated_at":"2026-11-20T16:02:28Z","downloads":{DOWNLOADS},"show":{{"tmdb_id":1,"name":"Show","overview":""}}}}"#
        );
        assert_eq!(parse(&minimal_show).title, "Show");
    }

    fn movie_json(show: &str) -> String {
        format!(
            r#"{{"title":"Show","variant":"movie","created_at":"2026-11-20T16:02:24Z",
            "updated_at":"2026-11-20T16:02:28Z","downloads":{DOWNLOADS},"show":{show}}}"#
        )
    }

    #[test]
    fn show_is_converted() {
        let show = parse(&movie_json(
            r#"{"tmdb_id":1,"tvdb_id":2,"name":"Show","romaji_name":"Shouu","original_name":"ショー","overview":"About","poster_url":"https://p/x.jpg","backdrop_url":"https://b/x.jpg"}"#,
        ))
        .show;
        assert_eq!(
            show,
            Some(domain::Show {
                tmdb_id: 1,
                tvdb_id: Some(2),
                name: "Show".into(),
                romaji_name: Some("Shouu".into()),
                original_name: Some("ショー".into()),
                overview: Some("About".into()),
                poster_url: Some("https://p/x.jpg".into()),
                backdrop_url: Some("https://b/x.jpg".into()),
            })
        );
    }

    #[test]
    fn show_treats_empty_values_as_absent() {
        let show = parse(&movie_json(
            r#"{"tmdb_id":1,"name":"Show","romaji_name":"","original_name":" ","overview":"  ","poster_url":"","backdrop_url":null}"#,
        ))
        .show
        .unwrap();
        assert_eq!(show.tvdb_id, None);
        assert_eq!(show.romaji_name, None);
        assert_eq!(show.original_name, None);
        assert_eq!(show.overview, None);
        assert_eq!(show.poster_url, None);
        assert_eq!(show.backdrop_url, None);
    }

    #[test]
    fn show_drops_invalid_urls_but_keeps_the_rest() {
        let show = parse(&movie_json(
            r#"{"tmdb_id":1,"name":"Show","overview":"About","poster_url":"not a url","backdrop_url":"ftp://b/x.jpg"}"#,
        ))
        .show
        .unwrap();
        assert_eq!(show.overview.as_deref(), Some("About"));
        assert_eq!(show.poster_url, None);
        assert_eq!(show.backdrop_url, None);
    }

    #[test]
    fn malformed_show_does_not_fail_the_message() {
        for show in [r#"{"name":5}"#, r#""text""#, "null", "[]"] {
            let collection = parse(&movie_json(show));
            assert_eq!(collection.show, None, "{show}");
            assert_eq!(collection.title, "Show");
        }
    }

    #[test]
    fn missing_or_invalid_fields_are_errors() {
        let missing_created = format!(
            r#"{{"title":"Show","variant":"movie","updated_at":"2026-11-20T16:02:28Z","downloads":{DOWNLOADS}}}"#
        );
        assert!(serde_json::from_str::<DownloadCollection>(&missing_created).is_err());

        let bad_timestamp = format!(
            r#"{{"title":"Show","variant":"movie","created_at":"yesterday",
            "updated_at":"2026-11-20T16:02:28Z","downloads":{DOWNLOADS}}}"#
        );
        assert!(serde_json::from_str::<DownloadCollection>(&bad_timestamp).is_err());

        let unknown_variant = format!(
            r#"{{"title":"Show","variant":"ova","created_at":"2026-11-20T16:02:24Z",
            "updated_at":"2026-11-20T16:02:28Z","downloads":{DOWNLOADS}}}"#
        );
        assert!(serde_json::from_str::<DownloadCollection>(&unknown_variant).is_err());
    }

    #[test]
    fn resolution_out_of_range_fails_conversion() {
        let json = r#"{"title":"Show","variant":"movie","created_at":"2026-11-20T16:02:24Z",
            "updated_at":"2026-11-20T16:02:28Z","downloads":[{"comments":"c","resolution":70000,
            "torrent":"t","file_name":"f","published_date":"2026-11-20T16:02:28Z"}]}"#;
        let dto: DownloadCollection = serde_json::from_str(json).unwrap();
        assert!(domain::DownloadCollection::try_from(dto).is_err());
    }
}
