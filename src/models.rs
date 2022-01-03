use crate::schema::*;
use std::convert::TryFrom;

#[derive(Queryable)]
pub struct Subscription {
    pub anime_id: i32,
    pub channel_id: i64,
    pub guild_id: i64,
}

#[derive(Queryable, Identifiable, Insertable, AsChangeset)]
#[table_name = "anime"]
pub struct Anime {
    pub id: i32,
    pub slug: String,
    pub synopsis: Option<String>,
    pub description: Option<String>,
    pub abbreviated_titles: Option<Vec<String>>,
    pub rating: Option<f32>,
    pub show_type: Option<String>,
    pub sub_type: Option<String>,
    pub episode_count: Option<i16>,
}

impl TryFrom<kitsu::models::Anime> for Anime {
    type Error = Box<dyn std::error::Error>;

    fn try_from(anime: kitsu::models::Anime) -> Result<Self, Self::Error> {
        Ok(Anime {
            id: anime.id.parse()?,
            slug: anime.slug,
            synopsis: anime.synopsis,
            description: anime.description,
            abbreviated_titles: Option::from(anime.abbreviated_titles),
            rating: anime.rating.and_then(|s| s.parse().ok()),
            show_type: Option::from(anime.show_type),
            sub_type: anime.sub_type,
            episode_count: anime.episode_count.and_then(|i| Option::from(i as i16)),
        })
    }
}

impl TryFrom<&kitsu::models::Anime> for Anime {
    type Error = Box<dyn std::error::Error>;

    fn try_from(anime: &kitsu::models::Anime) -> Result<Self, Self::Error> {
        Ok(Anime {
            id: anime.id.parse()?,
            slug: anime.slug.clone(),
            synopsis: anime.synopsis.clone(),
            description: anime.description.clone(),
            abbreviated_titles: Option::from(anime.abbreviated_titles.clone()),
            rating: anime.rating.as_ref().and_then(|s| s.parse().ok()),
            show_type: Option::from(anime.show_type.clone()),
            sub_type: None,
            episode_count: None,
        })
    }
}

#[derive(Queryable, Identifiable, Insertable)]
#[table_name = "anime_images"]
#[primary_key(anime_id, image_type, size)]
pub struct AnimeImage {
    pub anime_id: i32,
    pub image_type: String,
    pub size: String,
    pub url: String,
}

#[derive(Insertable, AsChangeset, Debug)]
#[table_name = "anime_images"]
pub struct AnimeImageInsert<'a> {
    pub anime_id: &'a i32,
    pub image_type: &'a str,
    pub size: &'a str,
    pub url: &'a str,
}

#[derive(Queryable, Identifiable)]
#[table_name = "anime_titles"]
#[primary_key(anime_id, language)]
pub struct AnimeTitle {
    pub anime_id: i32,
    pub language: String,
    pub title: String,
}

#[derive(Queryable, Identifiable, Insertable, AsChangeset)]
#[table_name = "anime_titles"]
#[primary_key(anime_id, language)]
pub struct AnimeTitleInsert<'a> {
    pub anime_id: &'a i32,
    pub language: &'a str,
    pub title: &'a str,
}
