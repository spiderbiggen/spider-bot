CREATE TABLE IF NOT EXISTS user_balance
(
    guild_id   BIGINT      NOT NULL,
    user_id    BIGINT      NOT NULL,
    balance    BIGINT      NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (guild_id, user_id)
)