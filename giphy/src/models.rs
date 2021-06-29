use strum_macros::{EnumString, IntoStaticStr};

#[derive(Deserialize, Debug)]
pub struct Response<T> {
    pub data: T,
}

#[derive(Deserialize, Debug)]
pub struct Gif {
    pub id: String,
    pub url: String,
    pub title: String,
    pub embed_url: String,
    pub rating: String,
}

#[derive(Debug, Copy, Clone, PartialEq, EnumString, IntoStaticStr)]
pub enum ContentFilter {
    #[strum(serialize = "g")]
    High,
    #[strum(serialize = "pg")]
    Medium,
    #[strum(serialize = "pg13")]
    Low,
    #[strum(serialize = "r")]
    Off,
}
