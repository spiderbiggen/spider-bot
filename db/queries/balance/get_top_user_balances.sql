SELECT user_id, balance
FROM user_balance
WHERE guild_id = $1
ORDER BY balance DESC
LIMIT 10;