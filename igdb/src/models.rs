use chrono::serde::ts_seconds_option::deserialize as ts_seconds_option;
use chrono::{DateTime, Utc};
use rustc_hash::FxHashMap;
use serde::Deserialize;
use serde_json::Value;
use std::fmt::Debug;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;
use url::Url;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct AccessToken {
    pub(crate) access_token: Arc<str>,
    pub(crate) expires_at: Instant,
}

#[derive(Deserialize)]
pub struct GameId(pub NonZeroU32);

impl Debug for GameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Deserialize)]
pub struct AgeRatingId(pub NonZeroU32);

impl Debug for AgeRatingId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Deserialize)]
pub struct AlterNativeNameId(pub NonZeroU32);

impl Debug for AlterNativeNameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Deserialize)]
pub struct GameTypeId(pub u32);

impl Debug for GameTypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Deserialize)]
pub struct Game {
    pub id: GameId,
    #[serde(default)]
    pub age_ratings: Vec<AgeRatingId>,
    /// Rating based on external critic scores
    pub aggregated_rating: Option<f64>,
    /// Number of external critic scores
    pub aggregated_rating_count: Option<u32>,
    /// AlternativeNames/properties/id
    #[serde(default)]
    pub alternative_names: Vec<AlterNativeNameId>,
    /// Artwork/properties/id
    #[serde(default)]
    pub artworks: Vec<u32>,
    #[serde(default)]
    pub bundles: Vec<u32>,
    /// GameType/properties/id
    pub game_type: Option<u32>,
    /// Collection/properties/id
    #[serde(default)]
    pub collections: Vec<u32>,
    /// Cover/properties/id
    pub cover: Option<u32>,
    /// Date this was initially added to the IGDB database
    #[serde(default, deserialize_with = "ts_seconds_option")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub dlcs: Vec<GameId>,
    #[serde(default)]
    pub expanded_games: Vec<GameId>,
    #[serde(default)]
    pub expansions: Vec<GameId>,
    /// ExternalGame/properties/id
    #[serde(default)]
    pub external_games: Vec<u32>,
    /// The first release date for this game
    #[serde(default, deserialize_with = "ts_seconds_option")]
    pub first_release_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub forks: Vec<GameId>,
    /// The main franchise
    pub franchise: Option<Box<str>>,
    /// Other franchises the game belongs to
    #[serde(default)]
    pub franchises: Vec<GameId>,
    /// The game engine used in this game
    ///
    /// GameEngine/properties/id
    #[serde(default)]
    pub game_engines: Vec<u32>,
    /// Supported game localizations for this game. A region can have at most one game localization for a given game
    ///
    /// GameLocalization/properties/id
    #[serde(default)]
    pub game_localizations: Vec<u32>,
    /// Modes of gameplay
    ///
    /// GameMode/properties/id
    #[serde(default)]
    pub game_modes: Vec<u32>,
    /// Genres of the game
    ///
    /// Genre/properties/id
    #[serde(default)]
    pub genres: Vec<u32>,
    /// Number of follows a game gets before release
    pub hypes: Option<u32>,
    /// Company/properties/id
    #[serde(default)]
    pub involved_companies: Vec<u32>,
    /// Keyword/properties/id
    #[serde(default)]
    pub keywords: Vec<u32>,
    /// LanguageSupport/properties/id
    #[serde(default)]
    pub language_supports: Vec<u32>,
    /// MultiplayerMode/properties/id
    #[serde(default)]
    pub multiplayer_modes: Vec<u32>,
    /// The name of the game
    pub name: Option<Box<str>>,
    /// If a DLC, expansion, or part of a bundle, this is the main game or bundle
    pub parent_game: Option<GameId>,
    /// Platforms this game was released on
    ///
    /// Platform/properties/id
    #[serde(default)]
    pub platforms: Vec<u32>,
    /// The main perspective of the player
    ///
    /// PlayerPerspectives/properties/id
    #[serde(default)]
    pub player_perspectives: Vec<u32>,
    /// Ports of this game
    #[serde(default)]
    pub ports: Vec<GameId>,
    /// Average IGDB user ratings
    pub rating: Option<f64>,
    /// Total number of IGDB user ratings
    pub rating_count: Option<u32>,
    /// Release dates of this game
    ///
    /// ReleaseDate/properties/id
    #[serde(default)]
    pub release_dates: Vec<u32>,
    /// Remakes of this game
    #[serde(default)]
    pub remakes: Vec<GameId>,
    /// Remasters of this game
    #[serde(default)]
    pub remasters: Vec<GameId>,
    /// Screenshots of this game
    ///
    /// Screenshots/properties/id
    #[serde(default)]
    pub screenshots: Vec<u32>,
    /// Similar games
    #[serde(default)]
    pub similar_games: Vec<u32>,
    /// A url-safe, unique, lower-case version of the name
    pub slug: Option<Box<str>>,
    /// Standalone expansions of this game
    #[serde(default)]
    pub standalone_expansions: Vec<GameId>,
    /// GameStatus/properties/id
    pub game_status: Option<u32>,
    /// A short description of a games story
    pub storyline: Option<Box<str>>,
    /// A description of the game
    pub summary: Option<Box<str>>,
    /// Related entities in the IGDB database
    #[serde(default)]
    pub tags: Vec<u32>,
    /// Themes of the game
    ///
    /// Theme/properties/id
    #[serde(default)]
    pub themes: Vec<u32>,
    /// Average rating based on both IGDB user and external critic scores
    pub total_rating: Option<f64>,
    /// Total number of user and external critic scores
    pub total_rating_count: Option<u32>,
    /// The last date this entry was updated in the IGDB database
    #[serde(default, deserialize_with = "ts_seconds_option")]
    pub updated_at: Option<DateTime<Utc>>,
    /// The website address (URL) of the item
    pub url: Option<Url>,
    /// If a version, this is the main game
    pub version_parent: Option<GameId>,
    /// Title of this version (i.e., Gold edition)
    pub version_title: Option<Box<str>>,
    /// Videos of this game
    ///
    /// GameVideo/properties/id
    #[serde(default)]
    pub videos: Vec<u32>,
    /// Websites associated with this game
    ///
    /// Website/properties/id
    #[serde(default)]
    pub websites: Vec<u32>,
    /// Hash of the object
    pub checksum: Option<Uuid>,
    #[serde(flatten)]
    pub extra: FxHashMap<String, Value>,
}

impl Game {
    #[must_use]
    pub const fn core_fields() -> [&'static str; 6] {
        [
            "name",
            "slug",
            "url",
            "created_at",
            "updated_at",
            "checksum",
        ]
    }
}

impl Debug for Game {
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = fmt.debug_struct("Game");
        f.field("id", &self.id);
        if let Some(name) = &self.name {
            f.field("name", name);
        }
        if let Some(slug) = &self.slug {
            f.field("slug", slug);
        }
        if let Some(game_type) = &self.game_type {
            f.field("game_type", game_type);
        }
        if let Some(created_at) = &self.created_at {
            f.field("created_at", created_at);
        }
        if let Some(updated_at) = &self.updated_at {
            f.field("updated_at", updated_at);
        }
        if let Some(url) = &self.url {
            f.field("url", &url.as_str());
        }
        if let Some(checksum) = &self.checksum {
            f.field("checksum", checksum);
        }
        if !self.age_ratings.is_empty() {
            f.field("age_ratings", &self.age_ratings);
        }
        if let Some(aggregated_rating) = &self.aggregated_rating {
            f.field("aggregated_rating", aggregated_rating);
        }
        if let Some(aggregated_rating_count) = &self.aggregated_rating_count {
            f.field("aggregated_rating_count", aggregated_rating_count);
        }
        if !self.alternative_names.is_empty() {
            f.field("alternative_names", &self.alternative_names);
        }
        if !self.artworks.is_empty() {
            f.field("artworks", &self.artworks);
        }
        if !self.bundles.is_empty() {
            f.field("bundles", &self.bundles);
        }
        if !self.collections.is_empty() {
            f.field("collections", &self.collections);
        }
        if let Some(cover) = &self.cover {
            f.field("cover", cover);
        }
        if !self.dlcs.is_empty() {
            f.field("dlcs", &self.dlcs);
        }
        if !self.expanded_games.is_empty() {
            f.field("expanded_games", &self.expanded_games);
        }
        if !self.expansions.is_empty() {
            f.field("expansions", &self.expansions);
        }
        if !self.external_games.is_empty() {
            f.field("external_games", &self.external_games);
        }
        if let Some(first_release_date) = &self.first_release_date {
            f.field("first_release_date", first_release_date);
        }
        if !self.forks.is_empty() {
            f.field("forks", &self.forks);
        }
        if let Some(franchise) = &self.franchise {
            f.field("franchise", franchise);
        }
        if !self.franchises.is_empty() {
            f.field("franchises", &self.franchises);
        }
        if !self.game_engines.is_empty() {
            f.field("game_engines", &self.game_engines);
        }
        if !self.game_localizations.is_empty() {
            f.field("game_localizations", &self.game_localizations);
        }
        if !self.game_modes.is_empty() {
            f.field("game_modes", &self.game_modes);
        }
        if !self.genres.is_empty() {
            f.field("genres", &self.genres);
        }
        if let Some(hypes) = &self.hypes {
            f.field("hypes", hypes);
        }
        if !self.involved_companies.is_empty() {
            f.field("involved_companies", &self.involved_companies);
        }
        if !self.keywords.is_empty() {
            f.field("keywords", &self.keywords);
        }
        if !self.language_supports.is_empty() {
            f.field("language_supports", &self.language_supports);
        }
        if !self.multiplayer_modes.is_empty() {
            f.field("multiplayer_modes", &self.multiplayer_modes);
        }
        if let Some(parent_game) = &self.parent_game {
            f.field("parent_game", parent_game);
        }
        if !self.platforms.is_empty() {
            f.field("platforms", &self.platforms);
        }
        if !self.player_perspectives.is_empty() {
            f.field("player_perspectives", &self.player_perspectives);
        }
        if !self.ports.is_empty() {
            f.field("ports", &self.ports);
        }
        if let Some(rating) = &self.rating {
            f.field("rating", rating);
        }
        if let Some(rating_count) = &self.rating_count {
            f.field("rating_count", rating_count);
        }
        if !self.release_dates.is_empty() {
            f.field("release_dates", &self.release_dates);
        }
        if !self.remakes.is_empty() {
            f.field("remakes", &self.remakes);
        }
        if !self.remasters.is_empty() {
            f.field("remasters", &self.remasters);
        }
        if !self.screenshots.is_empty() {
            f.field("screenshots", &self.screenshots);
        }
        if !self.similar_games.is_empty() {
            f.field("similar_games", &self.similar_games);
        }
        if !self.standalone_expansions.is_empty() {
            f.field("standalone_expansions", &self.standalone_expansions);
        }
        if let Some(game_status) = &self.game_status {
            f.field("game_status", &game_status);
        }
        if let Some(storyline) = &self.storyline {
            f.field("storyline", storyline);
        }
        if let Some(summary) = &self.summary {
            f.field("summary", summary);
        }
        if !self.tags.is_empty() {
            f.field("tags", &self.tags);
        }
        if !self.themes.is_empty() {
            f.field("themes", &self.themes);
        }
        if let Some(total_rating) = &self.total_rating {
            f.field("total_rating", &total_rating);
        }
        if let Some(total_rating_count) = self.total_rating_count {
            f.field("total_rating_count", &total_rating_count);
        }
        if let Some(version_parent) = &self.version_parent {
            f.field("version_parent", version_parent);
        }
        if let Some(version_title) = &self.version_title {
            f.field("version_title", version_title);
        }
        if !self.videos.is_empty() {
            f.field("videos", &self.videos);
        }
        if !self.websites.is_empty() {
            f.field("websites", &self.websites);
        }
        if !self.extra.is_empty() {
            f.field("extra", &self.extra);
        }
        f.finish()
    }
}

#[derive(Deserialize)]
pub struct GameType {
    pub id: GameTypeId,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "ts_seconds_option")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "ts_seconds_option")]
    pub updated_at: Option<DateTime<Utc>>,
    pub checksum: Option<Uuid>,
}

impl GameType {
    #[must_use]
    pub const fn core_fields() -> [&'static str; 4] {
        ["type", "created_at", "updated_at", "checksum"]
    }
}

impl Debug for GameType {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = fmt.debug_struct("GameType");
        f.field("id", &self.id);
        if let Some(kind) = &self.kind {
            f.field("kind", kind);
        }
        if let Some(created_at) = &self.created_at {
            f.field("created_at", created_at);
        }
        if let Some(updated_at) = &self.updated_at {
            f.field("updated_at", updated_at);
        }
        if let Some(checksum) = &self.checksum {
            f.field("checksum", checksum);
        }
        f.finish()
    }
}
