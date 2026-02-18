use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};
use strum_macros::{EnumString, IntoStaticStr};
use url::Url;

#[derive(Deserialize, Debug)]
pub struct Response<T> {
    pub result: bool,
    pub data: Data<T>,
}

#[derive(Deserialize, Debug)]
pub struct Data<T> {
    pub data: Vec<T>,
    pub current_page: u32,
    pub per_page: u8,
    pub has_next: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Gif {
    pub id: u64,
    pub slug: String,
    pub title: String,
    pub file: File,
}

impl Gif {
    #[must_use]
    pub fn media(&self, filter: Format) -> Option<&Url> {
        self.file.hd.get(&filter).map(|image| &image.url)
    }

    #[must_use]
    pub fn into_media(mut self, filter: Format) -> Option<Url> {
        self.file.hd.remove(&filter).map(|image| image.url)
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct File {
    pub hd: FxHashMap<Format, Image>,
}

#[derive(Deserialize, Clone)]
pub struct Image {
    pub url: Url,
    pub width: u32,
    pub height: u32,
    pub size: u32,
}

impl Debug for Image {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image")
            .field("url", &self.url.as_str())
            .field("width", &self.width)
            .field("height", &self.height)
            .field("size", &self.size)
            .finish()
    }
}

/// Klipy supports filtering content based on ratings that map to the Motion Picture Association (MPA)
/// It's important to note that klipy doesn't surface the type of nudity that can be found in R-rated films.
/// If you become aware of such content, inform Klipy immediately by contacting support@klipy.com.
#[derive(Debug, Copy, Clone, PartialEq, EnumString, IntoStaticStr, Default)]
pub enum ContentFilter {
    /// Rated G, PG, PG-13 and R (no nudity)
    #[strum(serialize = "off")]
    #[default]
    Off,
    /// Rated G, PG, and PG-13
    #[strum(serialize = "low")]
    Low,
    /// Rated G and PG
    #[strum(serialize = "medium")]
    Medium,
    /// Rated G
    #[strum(serialize = "high")]
    High,
}

impl From<ContentFilter> for Cow<'static, str> {
    fn from(value: ContentFilter) -> Self {
        Self::Borrowed(value.into())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, EnumString, IntoStaticStr, Deserialize)]
pub enum Format {
    #[strum(serialize = "gif")]
    #[serde(rename = "gif")]
    Gif,
    #[strum(serialize = "mp4")]
    #[serde(rename = "mp4")]
    Mp4,
    #[strum(serialize = "webm")]
    #[serde(rename = "webm")]
    Webm,
    #[strum(serialize = "webp")]
    #[serde(rename = "webp")]
    Webp,
    #[strum(serialize = "jpg")]
    #[serde(rename = "jpg")]
    Jpg,
}
