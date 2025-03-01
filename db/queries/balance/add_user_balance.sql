UPDATE user_balance
SET balance    = balance + $3,
    updated_at = NOW()
WHERE guild_id = $1 AND user_id = $2
RETURNING balance;