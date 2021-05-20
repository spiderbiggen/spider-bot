use strum_macros::{EnumString, IntoStaticStr};

#[derive(Deserialize, Debug)]
pub struct Response<T> {
    pub results: T,
}

#[derive(Deserialize, Debug)]
pub struct Gif {
    pub id: String,
    pub url: String,
    pub title: String,
}

#[derive(Debug, PartialEq, EnumString, IntoStaticStr)]
pub enum ContentFilter {
    #[strum(serialize = "high")]
    High,
    #[strum(serialize = "medium")]
    Medium,
    #[strum(serialize = "low")]
    Low,
    #[strum(serialize = "off")]
    Off,
}

#[derive(Debug, PartialEq, EnumString, IntoStaticStr)]
pub enum MediaFilter {
    #[strum(serialize = "basic")]
    Basic,
    #[strum(serialize = "minimal")]
    Minimal,
}

