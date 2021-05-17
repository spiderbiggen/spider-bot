table! {
    anime (id) {
        id -> Int4,
        slug -> Varchar,
        synopsis -> Text,
        description -> Text,
        abbreviated_titles -> Nullable<Array<Text>>,
        rating -> Nullable<Numeric>,
        show_type -> Nullable<Varchar>,
        sub_type -> Nullable<Varchar>,
        episode_count -> Nullable<Int2>,
    }
}

table! {
    anime_titles (anime_id, language) {
        anime_id -> Int4,
        language -> Varchar,
        title -> Varchar,
    }
}

joinable!(anime_titles -> anime (anime_id));

allow_tables_to_appear_in_same_query!(
    anime,
    anime_titles,
);
