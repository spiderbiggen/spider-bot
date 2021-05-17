#[derive(Deserialize, Debug)]
pub struct Results<T> {
    pub data: Vec<T>,
}

#[derive(Deserialize, Debug)]
pub struct SearchResult {
    pub id: String,
    pub url: String,
    pub title: String,
    pub embed_url: String,
    pub rating: String,
}