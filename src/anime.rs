use crate::util::edit_distance;
use chrono::{DateTime, TimeZone};
use kitsu::api;
use kitsu::models::Anime as KitsuAnime;
use nyaa::Anime;
use std::collections::HashMap;
use std::error::Error;

#[derive(PartialEq, Hash, Debug)]
pub(crate) struct AnimeGroup {
    pub(crate) title: String,
    pub(crate) episode: Option<i32>,
}

impl Eq for AnimeGroup {}

pub(crate) async fn update_from_nyaa<T: TimeZone>(
    prev_time: DateTime<T>,
) -> HashMap<AnimeGroup, Vec<Anime>> {
    let anime: Vec<Anime> = nyaa::get_anime().await;
    // let set: HashSet<String> = anime.iter().map(|a| a.title.to_string()).collect();
    // for title in set {
    //     println!("{:?}", get_anime(title).await)
    // }
    let current: Vec<Anime> = anime
        .into_iter()
        .filter(|item| item.pub_date > prev_time)
        .collect();
    println!("{:?}", current);
    let mut groups: HashMap<AnimeGroup, Vec<Anime>> = HashMap::new();
    current.into_iter().for_each(|anime| {
        let key = AnimeGroup {
            title: anime.title.clone(),
            episode: anime.episode,
        };
        let group = groups.entry(key).or_insert(vec![]);
        group.push(anime);
    });
    return groups;
}

pub(crate) async fn get_anime<S: AsRef<str>>(title: S) -> Result<Vec<KitsuAnime>, Box<dyn Error>> {
    let mut anime = api::anime::get_collection(title.as_ref()).await?;
    anime.sort_by_key(|a| edit_distance(&title, &a.canonical_title));
    Ok(anime)
}
