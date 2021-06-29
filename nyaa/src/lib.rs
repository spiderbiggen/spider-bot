#[macro_use]
extern crate lazy_static;

use std::cmp::max;
use std::error::Error;
use std::ops::Deref;

use chrono::{DateTime, FixedOffset};
use futures::future;
use regex::Regex;
use reqwest::Client;
use rss::{Channel, Item};
use tokio::task::JoinHandle;
use url::Url;

const TRACKERS: &[&str] = &[
    "http://nyaa.tracker.wf:7777/announce",
    "udp://open.stealth.si:80/announce",
    "udp://tracker.opentrackr.org:1337/announce",
    "udp://exodus.desync.com:6969/announce",
    "udp://tracker.torrent.eu.org:451/announce",
];
const BASE_URL: &str = "https://nyaa.si/?page=rss";

lazy_static! {
    static ref SOURCES: Vec<AnimeSource> = vec![AnimeSource::new(
        "[SubsPlease]",
        Some("1_2"),
        "^\\[.*?] (.*) - (\\d+)(?:\\.(\\d+))?(?:[vV](\\d+?))? \\((\\d+?p)\\) \\[.*?\\].mkv",
        None,
        vec!["(1080p)", "(720p)", "(540p)", "(480p)"]
    ),];
}

#[derive(Clone, Debug)]
pub struct AnimeSource {
    pub(crate) key: String,
    pub(crate) category: Option<String>,
    pub(crate) filter: Option<String>,
    pub(crate) regex: String,
    pub(crate) resolutions: Vec<String>,
}

impl AnimeSource {
    fn new<K>(
        key: K,
        category: Option<K>,
        regex: K,
        filter: Option<K>,
        resolutions: Vec<K>,
    ) -> AnimeSource
    where
        K: Into<String>,
    {
        AnimeSource {
            key: key.into(),
            category: category.and_then(|c| Some(c.into())),
            regex: regex.into(),
            filter: filter.and_then(|f| Some(f.into())),
            resolutions: resolutions.into_iter().map(|a| a.into()).collect(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AnimeTarget {
    pub(crate) title: String,
    pub(crate) resolution: Option<String>,
}

#[derive(Debug)]
pub struct Anime {
    pub(crate) id: Option<String>,
    pub title: String,
    pub episode: Option<i32>,
    pub decimal: Option<i32>,
    pub version: Option<i32>,
    pub comments: String,
    pub resolution: String,
    pub torrent: String,
    pub file_name: String,
    pub pub_date: DateTime<FixedOffset>,
}

impl Anime {
    pub fn get_magnet(&self) -> Option<String> {
        self.id.as_deref().and_then(|hash| {
            let mut url = Url::parse(format!("magnet:?urn:btih:{}", hash).as_str()).unwrap();
            let mut pairs = url.query_pairs_mut();
            for tracker in TRACKERS {
                pairs.append_pair("tr", tracker);
            }
            drop(pairs);
            let string = url.to_string();
            println!("{:?}", string);
            Some(string)
        })
    }
}

pub async fn get_anime() -> Vec<Anime> {
    println!("Fetching anime");
    let mut tasks: Vec<JoinHandle<Vec<Anime>>> = vec![];
    let client = Client::new();
    for source in SOURCES.clone() {
        let len = source.resolutions.len();
        (0..max(1, len))
            .filter_map(|i| build_url(&source, i))
            .map(|url| tokio::spawn(get_anime_for(client.clone(), url, source.clone())))
            .for_each(|handle| tasks.push(handle));
    }
    let joined = future::join_all(tasks).await;
    joined
        .into_iter()
        .filter(Result::is_ok)
        .flat_map(|item| item.unwrap())
        .collect()
}

async fn get_anime_for(client: Client, url: Url, source: AnimeSource) -> Vec<Anime> {
    let val = get_feed(client, &url).await.unwrap();
    println!("{}", val.title);

    return map_anime(val.items, &source);
}

fn build_url(provider: &AnimeSource, res_index: usize) -> Option<Url> {
    let mut filters: Vec<(&str, &str)> = Vec::new();

    let mut query: String = provider.key.to_string();
    if let Some(res) = provider.resolutions.get(res_index) {
        query.push(' ');
        query.push_str(res);
    }
    filters.push(("q", &query));
    if let Some(ref category) = provider.category {
        filters.push(("c", category.as_str()));
    }
    if let Some(ref filter) = provider.filter {
        filters.push(("f", filter.as_str()));
    }
    Url::parse_with_params(BASE_URL, filters).ok()
}

async fn get_feed(client: Client, url: &Url) -> Result<Channel, Box<dyn Error>> {
    let content = client.get(url.as_str()).send().await?.bytes().await?;
    let channel = Channel::read_from(&content[..])?;
    Ok(channel)
}

fn map_anime(items: Vec<Item>, source: &AnimeSource) -> Vec<Anime> {
    Regex::new(source.regex.as_str())
        .map(|regex| {
            items
                .into_iter()
                .filter_map(move |i| to_anime(i, &regex))
                .collect()
        })
        .unwrap_or(Vec::new())
}

#[derive(Debug, Eq, PartialEq)]
struct AnimeComponents(
    String,
    String,
    String,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

impl AnimeComponents {
    fn from_string<S>(inp: Option<S>, regex: &Regex) -> Option<AnimeComponents>
    where
        S: Into<String>,
    {
        inp.and_then(|s| Some(s.into()))
            .as_ref()
            .and_then(|title| regex.captures(title))
            .and_then(|cap| {
                let episode: Option<i32> = cap.get(2).and_then(|a| a.as_str().parse::<i32>().ok());
                let decimal: Option<i32> = cap.get(3).and_then(|a| a.as_str().parse::<i32>().ok());
                let version: Option<i32> = cap.get(4).and_then(|a| a.as_str().parse::<i32>().ok());
                let resolution: String = cap.get(5).unwrap().as_str().to_string();

                Some(AnimeComponents(
                    cap[0].into(),
                    cap[1].into(),
                    resolution,
                    episode,
                    decimal,
                    version,
                ))
            })
    }
}

fn to_anime(item: Item, regex: &Regex) -> Option<Anime> {
    let date = item
        .pub_date
        .as_ref()
        .and_then(|str| DateTime::parse_from_rfc2822(str).ok())?;
    let link = item.link?;
    let comments: String = item.guid?.value;
    let id: Option<String> = item
        .extensions
        .get("nyaa")
        .and_then(|nyaa| nyaa.get("infoHash"))
        .and_then(|info_hash| info_hash.deref().get(0))
        .and_then(|extension| extension.value.clone());

    AnimeComponents::from_string(item.title, regex).and_then(
        |AnimeComponents(file_name, title, resolution, episode, decimal, version)| {
            Some(Anime {
                id,
                episode,
                decimal,
                comments,
                version,
                resolution,
                title,
                file_name,
                torrent: link,
                pub_date: date,
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_anime_components_basic() {
        let input = "[_] Test Anime - 01 (1080p) [_].mkv";
        let expected = AnimeComponents(
            "[_] Test Anime - 01 (1080p) [_].mkv".into(),
            "Test Anime".into(),
            "1080p".into(),
            Some(1),
            None,
            None,
        );
        let source = SOURCES.get(0).unwrap();
        let regex = Regex::new(&source.regex).unwrap();
        let result = AnimeComponents::from_string(Some(input), &regex);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_anime_components_with_version_lower() {
        let input = "[_] Test Anime - 01v1 (1080p) [_].mkv";
        let expected = AnimeComponents(
            input.into(),
            "Test Anime".into(),
            "1080p".into(),
            Some(1),
            None,
            Some(1),
        );
        let source = SOURCES.get(0).unwrap();
        let regex = Regex::new(&source.regex).unwrap();
        let result = AnimeComponents::from_string(Some(input), &regex);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_anime_components_with_version_upper() {
        let input = "[_] Test Anime - 01V1 (1080p) [_].mkv";
        let expected = AnimeComponents(
            input.into(),
            "Test Anime".into(),
            "1080p".into(),
            Some(1),
            None,
            Some(1),
        );
        let source = SOURCES.get(0).unwrap();
        let regex = Regex::new(&source.regex).unwrap();
        let result = AnimeComponents::from_string(Some(input), &regex);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_anime_components_with_decimal() {
        let input = "[_] Test Anime - 01.1 (1080p) [_].mkv";
        let expected = AnimeComponents(
            input.into(),
            "Test Anime".into(),
            "1080p".into(),
            Some(1),
            Some(1),
            None,
        );
        let source = SOURCES.get(0).unwrap();
        let regex = Regex::new(&source.regex).unwrap();
        let result = AnimeComponents::from_string(Some(input), &regex);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_anime_components_with_decimal_and_version() {
        let input = "[_] Test Anime - 01.1V1 (1080p) [_].mkv";
        let expected = AnimeComponents(
            input.into(),
            "Test Anime".into(),
            "1080p".into(),
            Some(1),
            Some(1),
            Some(1),
        );
        let source = SOURCES.get(0).unwrap();
        let regex = Regex::new(&source.regex).unwrap();
        let result = AnimeComponents::from_string(Some(input), &regex);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_parse_anime_components_with_dash_in_title() {
        let input = "[_] Test-Anime - 01.1V1 (1080p) [_].mkv";
        let expected = AnimeComponents(
            input.into(),
            "Test-Anime".into(),
            "1080p".into(),
            Some(1),
            Some(1),
            Some(1),
        );
        let source = SOURCES.get(0).unwrap();
        let regex = Regex::new(&source.regex).unwrap();
        let result = AnimeComponents::from_string(Some(input), &regex);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), expected);
    }
}
