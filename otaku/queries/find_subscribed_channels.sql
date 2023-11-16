SELECT a.guild_id, a.channel_id
FROM anime_has_subscriptions a
WHERE a.partition_key = SUBSTRING($1, 0, 8)
  AND $1 ILIKE a.substring