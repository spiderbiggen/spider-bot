INSERT INTO user_balance (guild_id, user_id, balance, created_at, updated_at)
VALUES ($1, $2, $3, NOW(), NOW())
ON CONFLICT (guild_id, user_id)
    DO UPDATE SET balance    = user_balance.balance + $4,
                  updated_at = NOW()
RETURNING balance;
