use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Duration, TimeZone, Utc};
use serenity::client::Context;
use serenity::model::id::{ChannelId, GuildId};
use tokio::time;
use tokio::time::Instant;

use nyaa::Anime;

#[cfg(feature = "kitsu")]
use crate::commands::anime;

#[derive(PartialEq, Hash, Debug)]
struct AnimeGroup {
    pub(crate) title: String,
    pub(crate) episode: Option<i32>,
}

impl Eq for AnimeGroup {}

async fn update_from_nyaa<T: TimeZone>(prev_time: DateTime<T>) -> HashMap<AnimeGroup, Vec<Anime>> {
    let anime: Vec<Anime> = nyaa::get_anime().await;
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

pub(crate) async fn periodic_fetch(context: Arc<Context>, guilds: Arc<Vec<GuildId>>) {
    let mut last = Instant::now()
        .checked_sub(std::time::Duration::from_secs(1800))
        .unwrap_or(Instant::now());

    let mut interval_day = time::interval(std::time::Duration::from_secs(600));
    loop {
        let now = interval_day.tick().await;
        let prev_time = Utc::now()
            .checked_sub_signed(Duration::from_std(now.duration_since(last)).unwrap())
            .unwrap();

        let groups = update_from_nyaa(prev_time).await;
        let subscriptions = get_subscriptions_for_channel().await;

        for (group, anime) in groups {
            for guild in guilds.iter() {
                if let Some(sub) = subscriptions.get(guild) {
                    if let Some(channels) = sub.get(&group.title) {
                        for channel in channels {
                            send_anime_embed(&context, channel, &group, &anime).await;
                        }
                    }
                }
            }
        }
        last = now;
    }
}

async fn send_anime_embed(
    ctx: &Arc<Context>,
    channel: &ChannelId,
    group: &AnimeGroup,
    anime: &Vec<Anime>,
) {
    let image: Option<String> = if cfg!(feature = "kitsu") {
        match anime::get_anime(&group.title).await {
            Ok(collection) => collection.first().map_or(None, |a| {
                a.cover_image
                    .clone()
                    .or(a.poster_image.clone())
                    .map_or(None, |a| a.medium.or(a.original))
            }),
            Err(_) => None,
        }
    } else {
        None
    };

    let result = channel
        .send_message(&ctx, |m| {
            m.embed(|e| {
                if let Some(url) = image {
                    e.image(url);
                }
                e.title(format!(
                    "{} Ep {}",
                    &group.title,
                    group.episode.map_or("".to_string(), |a| a.to_string())
                ));
                anime.into_iter().for_each(|anime| {
                    e.field(
                        &anime.resolution,
                        format!(
                            "[torrent]({})\n[comments]({})",
                            &anime.torrent, &anime.comments
                        ),
                        true,
                    );
                });
                e
            })
        })
        .await;
    match result {
        Ok(m) => {
            if let Err(why) = channel.pin(&ctx, m).await {
                eprintln!("Error pinning message: {:?}", why);
            }
        }
        Err(why) => {
            eprintln!("Error sending message: {:?}", why);
        }
    }
}

async fn get_subscriptions_for_channel() -> HashMap<GuildId, HashMap<String, Vec<ChannelId>>> {
    let mut map: HashMap<GuildId, HashMap<String, Vec<ChannelId>>> = HashMap::new();

    map.insert(GuildId(825808364649971712), {
        let mut map: HashMap<String, Vec<ChannelId>> = HashMap::new();
        map.insert(
            "Kumo desu ga, Nani ka".to_string(),
            vec![ChannelId(825808364649971715)],
        );
        map.insert(
            "World Trigger S3".to_string(),
            vec![ChannelId(825808364649971715)],
        );
        map.insert(
            "Princess Connect! Re-Dive S2".to_string(),
            vec![ChannelId(825808364649971715)],
        );
        map.insert(
            "Girls und Panzer das Finale".to_string(),
            vec![ChannelId(825808364649971715)],
        );
        map
    });
    map.insert(GuildId(165162546444107776), {
        let mut map: HashMap<String, Vec<ChannelId>> = HashMap::new();
        map.insert(
            "Boku no Hero Academia".to_string(),
            vec![ChannelId(178167855718727680)],
        );
        map.insert(
            "Shingeki no Kyojin (The Final Season)".to_string(),
            vec![ChannelId(178167855718727680)],
        );
        map
    });

    map
}
