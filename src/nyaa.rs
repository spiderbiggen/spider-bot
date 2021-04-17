use std::cmp::max;
use std::error::Error;

use chrono::{DateTime, FixedOffset};
use futures::future;
use regex::Regex;
use reqwest::Client;
use rss::{Channel, Item};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
use url::Url;

#[derive(Deserialize, Clone, Debug)]
pub struct AnimeSource {
    pub(crate) key: String,
    pub(crate) category: Option<String>,
    pub(crate) filter: Option<String>,
    pub(crate) regex: Option<String>,
    #[serde(default)]
    pub(crate) resolutions: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AnimeTarget {
    pub(crate) title: String,
    pub(crate) resolution: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Anime {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,
    pub(crate) title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) episode: Option<i32>,
    pub(crate) comments: String,
    pub(crate) resolution: String,
    pub(crate) torrent: String,
    pub(crate) file_name: String,
    pub(crate) pub_date: DateTime<FixedOffset>,
}


lazy_static! {
    static ref FILE_REGEX: Regex = Regex::new(r"^\[.*?] (.*?)(?: - (\d+))? [(\[](.*?)[)\]]").unwrap();
    static ref BASE_URL: Url = Url::parse("https://nyaa.si/?page=rss").unwrap();
    static ref SOURCES: Vec<AnimeSource> = vec![
        AnimeSource {key: "[SubsPlease]".to_string(),category:Some("1_2".to_string()),regex: Some("^\\[.*?] (.*) - (\\d+) \\((\\d+?p)\\) \\[.*?\\].mkv".to_string()), resolutions: vec!["(1080p)".to_string(), "(720p)".to_string(), "(480p)".to_string()],filter: None},
    ];
}

// const TRACKERS: &[&str] = &[
//     "http://nyaa.tracker.wf:7777/announce",
//     "udp://open.stealth.si:80/announce",
//     "udp://tracker.opentrackr.org:1337/announce",
//     "udp://exodus.desync.com:6969/announce",
//     "udp://tracker.torrent.eu.org:451/announce"
// ];
//
// pub fn create_magnet_uri(anime: &Anime) -> Option<String> {
//     anime.id.as_deref().and_then(|hash| {
//         let mut url = Url::parse(format!("magnet:?urn:btih:{}", hash).as_str()).unwrap();
//         let mut pairs = url.query_pairs_mut();
//         for tracker in TRACKERS {
//             pairs.append_pair("tr", tracker);
//         }
//         drop(pairs);
//         url.to_owned();
//         let string = url.to_string();
//         println!("{:?}", string);
//         Some(string)
//     })
// }


pub async fn get_anime() -> Vec<Anime> {
    println!("Fetching anime");
    let mut tasks: Vec<JoinHandle<Vec<Anime>>> = vec![];
    let client = Client::new();
    for source in SOURCES.clone() {
        let len = source.resolutions.len();
        for i in 0..max(1, len) {
            let url = build_url(&source, i);
            let client = client.clone();
            let handle = tokio::spawn(get_anime_for(client, url, source.clone()));
            tasks.push(handle);
        }
    }
    let joined = future::join_all(tasks).await;
    joined.into_iter()
        .flat_map(|item| item.ok())
        .flatten()
        .collect()
}

async fn get_anime_for(client: Client, url: Url, source: AnimeSource) -> Vec<Anime> {
    let val = get_feed(client, &url).await.unwrap();
    println!("{}", val.title);

    return map_anime(val.items, &source);
}

fn build_url(provider: &AnimeSource, res_index: usize) -> Url {
    let mut url = BASE_URL.clone();
    let mut pairs = url.query_pairs_mut();
    let mut query: String = provider.key.to_string();
    if let Some(res) = provider.resolutions.get(res_index) {
        query.push(' ');
        query.push_str(res);
    }
    pairs.append_pair("q", &query);
    if let Some(ref category) = provider.category {
        pairs.append_pair("c", category.as_str());
    }
    if let Some(ref filter) = provider.filter {
        pairs.append_pair("f", filter.as_str());
    }
    drop(pairs);
    url.to_owned()
}

async fn get_feed(client: Client, url: &Url) -> Result<Channel, Box<dyn Error>> {
    let content = client.get(url.as_str())
        .send().await?
        .bytes().await?;
    let channel = Channel::read_from(&content[..])?;
    Ok(channel)
}

fn map_anime(items: Vec<Item>, source: &AnimeSource) -> Vec<Anime> {
    let regex= source.regex.clone().and_then(|r| Regex::new(r.as_str()).ok());
    items.iter()
        .map(|ref i| to_anime(i, regex.as_ref().unwrap_or(&FILE_REGEX)))
        .filter_map(|a| a.unwrap())
        .collect()
}

fn to_anime(item: &Item, regex: &Regex) -> Result<Option<Anime>, Box<dyn Error>> {
    let date = match item.pub_date {
        None => return Ok(None),
        Some(ref date_str) => DateTime::parse_from_rfc2822(&date_str)?,
    };
    let link = match item.link {
        None => return Ok(None),
        Some(ref link) => link,
    };
    let comments = match item.guid {
        None => return Ok(None),
        Some(ref guid) => guid.value.to_string(),
    };
    let mut id: Option<String> = None;
    if let Some(nyaa) = item.extensions.get("nyaa") {
        if let Some(info_hash) = nyaa.get("infoHash") {
            if let Some(extension) = info_hash.get(0) {
                if let Some(value) = extension.value.as_ref() {
                    id = Some(value.to_string())
                }
            }
        }
    };
    match item.title {
        None => return Ok(None),
        Some(ref title) => {
            if let Some(cap) = regex.captures(title) {
                let episode: Option<i32> = cap.get(2).and_then(|a| a.as_str().parse::<i32>().ok());
                let anime = Anime {
                    id,
                    episode,
                    comments,
                    title: cap[1].to_string(),
                    resolution: cap[3].to_string(),
                    file_name: title.to_string(),
                    torrent: link.to_string(),
                    pub_date: date,
                };
                Ok(Some(anime))
            } else {
                Ok(None)
            }
        }
    }
}