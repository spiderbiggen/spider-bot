//! Wire format of the anime service's `download` server-sent events.

use domain::DownloadVariant;
use serde::Deserialize;
use serde::de::IgnoredAny;
use std::num::TryFromIntError;

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
    /// Show metadata matched by the service. Accepted but not used yet.
    #[serde(default)]
    #[allow(dead_code)]
    show: Option<IgnoredAny>,
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
        })
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
