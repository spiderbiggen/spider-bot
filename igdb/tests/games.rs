use igdb::models::Game;
use igdb::{Limit, QueryBuilder};

mod common;

#[tokio::test]
async fn games() {
    let igdb = common::setup();
    let builder = QueryBuilder::new().all_fields();
    let games = dbg!(igdb.games(Some(&builder)).await.unwrap());
    assert!(!games.is_empty());
}

#[tokio::test]
async fn games_max_offset() {
    const LIMIT: Limit = Limit::new(10).unwrap();
    let igdb = common::setup();
    let builder = QueryBuilder::new()
        .sort_asc("id")
        .limit(LIMIT)
        .offset(u32::MAX);
    let games = dbg!(igdb.games(Some(&builder)).await.unwrap());
    assert!(games.is_empty());
}

#[tokio::test]
async fn games_sorted_by_id_asc() {
    let igdb = common::setup();
    let builder = QueryBuilder::new().sort_asc("id");
    let games = dbg!(igdb.games(Some(&builder)).await.unwrap());
    assert!(!games.is_empty());
}

#[tokio::test]
async fn games_all_fields_sorted_by_id_asc() {
    let igdb = common::setup();
    let builder = QueryBuilder::new().all_fields().sort_asc("id");
    let games = dbg!(igdb.games(Some(&builder)).await.unwrap());
    assert!(!games.is_empty());
}

#[tokio::test]
async fn games_core_fields_sorted_by_id_asc() {
    let igdb = common::setup();
    let builder = QueryBuilder::new()
        .fields(Game::core_fields())
        .sort_asc("id");
    let games = dbg!(igdb.games(Some(&builder)).await.unwrap());
    assert!(!games.is_empty());
}

#[tokio::test]
async fn games_core_fields_sorted_by_id_desc() {
    let igdb = common::setup();
    let builder = QueryBuilder::new()
        .fields(Game::core_fields())
        .sort_desc("id");
    let games = dbg!(igdb.games(Some(&builder)).await.unwrap());
    assert!(!games.is_empty());
}

#[tokio::test]
async fn games_all_fields_sorted_by_id_desc() {
    let igdb = common::setup();
    let builder = QueryBuilder::new().all_fields().sort_asc("id");
    let games = dbg!(igdb.games(Some(&builder)).await.unwrap());
    assert!(!games.is_empty());
}

#[tokio::test]
async fn games_released_all_fields_sorted_by_release_desc() {
    let igdb = common::setup();
    let now = chrono::Utc::now();
    let builder = QueryBuilder::new()
        .all_fields()
        .filter(format!(
            "first_release_date < {} & game_type = (0,4,8,9)",
            now.timestamp()
        ))
        .sort_desc("first_release_date");
    let games = dbg!(igdb.games(Some(&builder)).await.unwrap());
    assert!(!games.is_empty());
}

#[tokio::test]
async fn games_released_core_fields_sorted_by_release_desc() {
    const LIMIT: Limit = Limit::new(500).unwrap();

    let igdb = common::setup();
    let now = chrono::Utc::now();
    let builder = QueryBuilder::new()
        .fields(Game::core_fields())
        .fields(["summary"])
        .filter(format!(
            "first_release_date < {} & game_type = (0,4,8,9)",
            now.timestamp()
        ))
        .limit(LIMIT)
        .sort_desc("first_release_date");
    let games = dbg!(igdb.games(Some(&builder)).await.unwrap());
    println!("{}", games.len());
    assert!(!games.is_empty());
}
