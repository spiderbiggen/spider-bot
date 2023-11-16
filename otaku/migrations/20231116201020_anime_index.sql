ALTER TABLE anime_has_subscriptions
    ADD COLUMN partition_key VARCHAR(8);

UPDATE anime_has_subscriptions
SET partition_key = SUBSTRING(substring, 0, 8);

ALTER TABLE anime_has_subscriptions
    ALTER COLUMN partition_key SET NOT NULL;

CREATE INDEX idx_subscription_partition ON anime_has_subscriptions (partition_key);
