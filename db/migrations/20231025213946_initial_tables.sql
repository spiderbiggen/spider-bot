CREATE TABLE IF NOT EXISTS anime
(
    id              TEXT NOT NULL PRIMARY KEY,
    created_at      TIMESTAMP,
    canonical_title TEXT NOT NULL,
    query_title     TEXT NOT NULL,
    image_url       TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS anime_has_subscriptions
(
    guild_id   TEXT         NOT NULL,
    channel_id TEXT         NOT NULL,
    substring  VARCHAR(255) NOT NULL,
    anime_id   TEXT
);

