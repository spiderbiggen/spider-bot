use igdb::QueryBuilder;
use igdb::models::GameType;

mod common;

#[tokio::test]
async fn game_types() {
    let igdb = common::setup();
    let builder = QueryBuilder::new().all_fields();
    let game_types = dbg!(igdb.game_types(Some(&builder)).await.unwrap());
    assert!(!game_types.is_empty());
}

#[tokio::test]
async fn game_types_sorted_by_id_asc() {
    let igdb = common::setup();
    let builder = QueryBuilder::new().sort_asc("id");
    let game_types = dbg!(igdb.game_types(Some(&builder)).await.unwrap());
    assert!(!game_types.is_empty());
}

#[tokio::test]
async fn game_types_all_fields_sorted_by_id_asc() {
    let igdb = common::setup();
    let builder = QueryBuilder::new().all_fields().sort_asc("id");
    let game_types = dbg!(igdb.game_types(Some(&builder)).await.unwrap());
    assert!(!game_types.is_empty());
}

#[tokio::test]
async fn game_types_core_fields_sorted_by_id_asc() {
    let igdb = common::setup();
    let builder = QueryBuilder::new()
        .fields(GameType::core_fields())
        .sort_asc("id");
    let game_types = dbg!(igdb.game_types(Some(&builder)).await.unwrap());
    assert!(!game_types.is_empty());
}

#[tokio::test]
async fn game_types_core_fields_sorted_by_id_desc() {
    let igdb = common::setup();
    let builder = QueryBuilder::new()
        .fields(GameType::core_fields())
        .sort_desc("id");
    let game_types = dbg!(igdb.game_types(Some(&builder)).await.unwrap());
    assert!(!game_types.is_empty());
}

#[tokio::test]
async fn game_types_all_fields_sorted_by_id_desc() {
    let igdb = common::setup();
    let builder = QueryBuilder::new().all_fields().sort_asc("id");
    let game_types = dbg!(igdb.game_types(Some(&builder)).await.unwrap());
    assert!(!game_types.is_empty());
}
