use crate::context::Context;
use db::{BalanceTransactionError, UserBalanceConnection, UserBalanceTransaction};
use futures::StreamExt;
use poise::CreateReply;
use serenity::all::{CreateEmbed, Member, Permissions};
use std::num::{NonZeroI16, NonZeroU16};

const INITIAL_BALANCE: i64 = 500;

#[expect(clippy::unused_async)]
#[poise::command(
    slash_command,
    guild_only,
    subcommands("balance", "transfer", "leaderboard", "set", "update")
)]
pub(crate) async fn coin(_: Context<'_, '_>) -> Result<(), crate::commands::CommandError> {
    Ok(())
}

#[poise::command(slash_command, guild_only)]
pub(crate) async fn balance(ctx: Context<'_, '_>) -> Result<(), crate::commands::CommandError> {
    ctx.defer_ephemeral().await?;

    let Some(guild_id) = ctx.guild_id() else {
        return Ok(());
    };
    let guild_id = guild_id.get();
    let user_id = ctx.author().id.get();

    let db = &ctx.data().database;
    let (balance, is_new) = db
        .get_or_create_user_balance(guild_id, user_id, INITIAL_BALANCE)
        .await?;

    let message = if is_new {
        format!("Welcome to True Coin. You currently have {balance} 🪙")
    } else {
        format!("You currently have {balance} 🪙")
    };

    ctx.reply(message).await?;
    Ok(())
}

#[poise::command(slash_command, guild_only)]
pub(crate) async fn transfer(
    ctx: Context<'_, '_>,
    #[description = "Who to send coins to"] member: Member,
    #[description = "Amount of coins to send to another user"]
    #[max = 1000]
    amount: NonZeroU16,
) -> Result<(), crate::commands::CommandError> {
    if member.user.id == ctx.author().id {
        let reply = CreateReply::default()
            .ephemeral(true)
            .content("You cannot send coins to yourself.");
        ctx.send(reply).await?;
        return Ok(());
    }

    if member.user.bot {
        let reply = CreateReply::default()
            .ephemeral(true)
            .content("Bot users cannot handle the true power of coins.");
        ctx.send(reply).await?;
        return Ok(());
    }

    let db = &ctx.data().database;

    let Some(from_user) = ctx.author_member().await else {
        return Ok(());
    };
    let Some(guild_id) = ctx.guild_id() else {
        return Ok(());
    };
    let guild_id = guild_id.get();
    let result = db
        .transfer_user_balance(
            guild_id,
            from_user.user.id.get(),
            member.user.id.get(),
            i64::from(amount.get()),
        )
        .await;

    let (from_balance, to_balance) = match result {
        Err(err) => return handle_transfer_error(ctx, &member, err).await,
        Ok(result) => result,
    };

    let to_name = &member.display_name();
    let from_name = &from_user.display_name();
    let width = from_name.len().max(to_name.len());
    let message = format!(
        "```\nSuccessfully transferred {amount} 🪙 to {to_name}. New Balance:\n\
                {from_name:>width$}: {from_balance:>4} 🪙\n\
                {to_name:>width$}: {to_balance:>4} 🪙\n```",
    );
    ctx.say(message).await?;
    Ok(())
}

async fn handle_transfer_error(
    ctx: Context<'_, '_>,
    member: &Member,
    err: BalanceTransactionError,
) -> Result<(), crate::commands::CommandError> {
    let message = match err {
        BalanceTransactionError::Base(err) => return Err(err.into()),
        BalanceTransactionError::SenderUninitialized => {
            "Use `/coins balance` to initialize your coins.".to_string()
        }
        BalanceTransactionError::RecipientUninitialized => {
            format!(
                "Tell @{} to use `/coins balance` to initialize their coins.",
                member.display_name()
            )
        }
        BalanceTransactionError::InsufficientBalance(current_amount) => {
            format!("You do not have enough coins. Current balance {current_amount} 🪙")
        }
    };
    let reply = CreateReply::default().ephemeral(true).content(message);
    ctx.send(reply).await?;
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
struct MemberBalance {
    balance: i64,
    username: String,
}

#[poise::command(slash_command, guild_only)]
pub(crate) async fn leaderboard(ctx: Context<'_, '_>) -> Result<(), crate::commands::CommandError> {
    ctx.defer().await?;
    let db = &ctx.data().database;

    let Some(guild) = ctx.guild_id() else {
        return Ok(());
    };
    let guild_id = guild.get();
    let users = db.get_top_user_balances(guild_id).await?;
    if users.is_empty() {
        ctx.say("There are no users in the leaderboard.").await?;
        return Ok(());
    }

    let member_balances: Vec<_> = futures::stream::iter(users)
        .map(async |user_balance| {
            let member = guild.member(&ctx, user_balance.user_id).await.ok()?;
            if member.user.bot {
                return None;
            }

            Some(MemberBalance {
                username: member.display_name().to_string(),
                balance: user_balance.balance,
            })
        })
        .buffered(8)
        .filter_map(futures::future::ready)
        .collect()
        .await;

    if member_balances.is_empty() {
        ctx.say("There are no users in the leaderboard.").await?;
        return Ok(());
    }

    let description = member_balances
        .iter()
        .enumerate()
        .map(|(i, MemberBalance { username, balance })| {
            let rank = match i {
                0 => "🥇".to_string(),
                1 => "🥈".to_string(),
                2 => "🥉".to_string(),
                n => format!("{}.", n + 1),
            };
            format!("{rank} **{username}** — {} 🪙", format_balance(*balance))
        })
        .collect::<Vec<_>>()
        .join("\n");

    let embed = CreateEmbed::new()
        .title("🪙 True Coin Leaderboard")
        .description(description)
        .color(0x00FF_D700_u32);

    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}

#[allow(dead_code)]
async fn author_is_guild_admin(
    ctx: Context<'_, '_>,
) -> Result<bool, crate::commands::CommandError> {
    let Some(member) = ctx.author_member().await else {
        return Ok(false);
    };
    let allowed = ctx.framework().options.owners.contains(&member.user.id)
        || member.permissions.is_some_and(Permissions::administrator);
    Ok(allowed)
}

#[poise::command(slash_command, check = "author_is_guild_admin")]
pub(crate) async fn set(
    ctx: Context<'_, '_>,
    #[description = "Who to set coins for"] member: Member,
    #[description = "Amount of coins the user should have"]
    #[max = 999_999_999]
    amount: i32,
) -> Result<(), crate::commands::CommandError> {
    ctx.defer().await?;
    let db = &ctx.data().database;
    let Some(guild_id) = ctx.guild_id() else {
        return Ok(());
    };
    let guild_id = guild_id.get();
    let user_id = member.user.id.get();

    let amount = i64::from(amount);
    let amount = db
        .upsert_set_user_balance(guild_id, user_id, amount)
        .await?;
    let message = format!("{} now has {amount} 🪙", member.display_name());
    ctx.say(message).await?;
    Ok(())
}

#[poise::command(slash_command, check = "author_is_guild_admin")]
pub(crate) async fn update(
    ctx: Context<'_, '_>,
    #[description = "Who to update coins for"] member: Member,
    #[description = "Amount of coins the user should gain/lose"]
    #[min = -500]
    #[max = 500]
    amount: NonZeroI16,
) -> Result<(), crate::commands::CommandError> {
    ctx.defer().await?;
    let db = &ctx.data().database;
    let Some(guild_id) = ctx.guild_id() else {
        return Ok(());
    };
    let guild_id = guild_id.get();
    let user_id = member.user.id.get();

    let amount = i64::from(amount.get());
    let balance = db
        .upsert_update_user_balance(guild_id, user_id, amount, INITIAL_BALANCE + amount)
        .await?;
    let message = format!(
        "{} now has {balance} ({amount:+}) 🪙",
        member.display_name()
    );
    ctx.say(message).await?;
    Ok(())
}

/// Format a coin balance with thousands separators (e.g. `1234567` → `"1,234,567"`).
fn format_balance(n: i64) -> String {
    let digits: Vec<char> = n.unsigned_abs().to_string().chars().rev().collect();
    let mut parts: Vec<String> = digits
        .chunks(3)
        .map(|chunk| chunk.iter().rev().collect::<String>())
        .collect();
    parts.reverse();
    let formatted = parts.join(",");
    if n < 0 {
        format!("-{formatted}")
    } else {
        formatted
    }
}
