use nyaa::Anime;
use std::collections::HashMap;
use chrono::{DateTime, TimeZone};

#[derive(PartialEq, Hash, Debug)]
pub(crate) struct AnimeGroup {
    pub(crate) title: String,
    pub(crate) episode: Option<i32>,
}

impl Eq for AnimeGroup {}

pub(crate) async fn update_from_nyaa<T: TimeZone>(prev_time: DateTime<T>) -> HashMap<AnimeGroup, Vec<Anime>> {
    let anime: Vec<Anime> = nyaa::get_anime().await;
    let current: Vec<Anime> = anime.into_iter().filter(|item| item.pub_date > prev_time).collect();
    println!("{:?}", current);
    let mut groups: HashMap<AnimeGroup, Vec<Anime>> = HashMap::new();
    current.into_iter().for_each(|anime| {
        let key = AnimeGroup { title: anime.title.clone(), episode: anime.episode };
        let group = groups.entry(key).or_insert(vec![]);
        group.push(anime);
    });
    return groups;
}