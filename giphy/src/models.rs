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
