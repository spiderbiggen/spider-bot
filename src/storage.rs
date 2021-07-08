use std::env;

use diesel::{Connection, PgConnection};

pub fn establish_connection() -> PgConnection {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url).expect(&format!("Error connecting to {}", database_url))
}

pub mod anime {
    use crate::models::*;
    use diesel;
    use diesel::{PgConnection, QueryResult, RunQueryDsl};
    use kitsu::models::{Anime as KitsuAnime, ImageSet};
    use std::collections::HashMap;
    use std::convert::TryFrom;

    pub fn insert_kitsu_anime(conn: &PgConnection, anime: &Vec<KitsuAnime>) -> QueryResult<()> {
        let values: Vec<Anime> = anime
            .iter()
            .filter_map(|s| Anime::try_from(s).ok())
            .collect();

        for (anime, k_anime) in values.iter().zip(anime) {
            insert_anime(conn, anime)?;
            insert_anime_titles(conn, anime, &k_anime.titles)?;
            k_anime
                .cover_image
                .as_ref()
                .and_then(|i| insert_anime_images(conn, anime, "CoverImage", i).ok());
            k_anime
                .poster_image
                .as_ref()
                .and_then(|i| insert_anime_images(conn, anime, "PosterImage", i).ok());
        }
        Ok(())
    }

    pub fn insert_anime(conn: &PgConnection, a: &Anime) -> QueryResult<()> {
        use crate::schema::anime;
        use crate::schema::anime::dsl::*;
        diesel::insert_into(anime::table)
            .values(a)
            .on_conflict(id)
            .do_update()
            .set(a)
            .execute(conn)?;
        Ok(())
    }

    pub fn insert_anime_titles(
        conn: &PgConnection,
        anime: &Anime,
        titles: &HashMap<String, String>,
    ) -> QueryResult<()> {
        use crate::schema::anime_titles;
        use crate::schema::anime_titles::dsl::*;
        let values: Vec<AnimeTitleInsert> = titles
            .iter()
            .map(|(key, val)| AnimeTitleInsert {
                anime_id: &anime.id,
                language: key,
                title: val,
            })
            .collect();
        for value in values {
            diesel::insert_into(anime_titles::table)
                .values(&value)
                .on_conflict((anime_id, language))
                .do_update()
                .set(&value)
                .execute(conn)?;
        }
        Ok(())
    }

    pub fn insert_anime_images(
        conn: &PgConnection,
        anime: &Anime,
        _type: &str,
        images: &ImageSet,
    ) -> QueryResult<()> {
        use crate::schema::anime_images;
        use crate::schema::anime_images::dsl::*;
        let id = &anime.id;
        let values: Vec<AnimeImageInsert> = vec![
            images.tiny.as_ref().and_then(|i| {
                Some(AnimeImageInsert {
                    anime_id: id,
                    image_type: _type,
                    size: "tiny",
                    url: i,
                })
            }),
            images.small.as_ref().and_then(|i| {
                Some(AnimeImageInsert {
                    anime_id: id,
                    image_type: _type,
                    size: "small",
                    url: i,
                })
            }),
            images.medium.as_ref().and_then(|i| {
                Some(AnimeImageInsert {
                    anime_id: id,
                    image_type: _type,
                    size: "medium",
                    url: i,
                })
            }),
            images.original.as_ref().and_then(|i| {
                Some(AnimeImageInsert {
                    anime_id: id,
                    image_type: _type,
                    size: "original",
                    url: i,
                })
            }),
            images.large.as_ref().and_then(|i| {
                Some(AnimeImageInsert {
                    anime_id: id,
                    image_type: _type,
                    size: "large",
                    url: i,
                })
            }),
        ]
        .into_iter()
        .filter_map(|s| s)
        .collect();
        println!("{:?}", values);
        for value in values {
            let a = diesel::insert_into(anime_images::table)
                .values(&value)
                .on_conflict((anime_id, image_type, size))
                .do_update()
                .set(&value)
                .execute(conn);
            match a {
                Ok(_) => {}
                Err(e) => println!("{:?}", e),
            }
        }
        Ok(())
    }
}
