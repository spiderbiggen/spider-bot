#[derive(Queryable)]
pub struct Subscription {
    pub title: String,
    pub channel_id: i64,
    pub guild_id: i64,
}