use crate::context::Context;
use db::{BalanceTransactionError, UserBalanceConnection, UserBalanceTransaction};
use futures::StreamExt;
use poise::CreateReply;
use serenity::all::{Member, Permissions};
use std::fmt::Write;
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

#[poise::command(slash_command)]
pub(crate) async fn balance(ctx: Context<'_, '_>) -> Result<(), crate::commands::CommandError> {
    ctx.defer_ephemeral().await?;

    let guild_id = ctx.guild_id().unwrap().get();
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

#[poise::command(slash_command)]
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
    let guild_id = ctx.guild_id().unwrap().get();
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

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
struct MemberBalance {
    balance: i64,
    username: String,
}

#[poise::command(slash_command)]
pub(crate) async fn leaderboard(ctx: Context<'_, '_>) -> Result<(), crate::commands::CommandError> {
    ctx.defer().await?;
    let db = &ctx.data().database;

    let guild = ctx.guild_id().unwrap();
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

    let width = member_balances
        .iter()
        .map(|m| m.username.len())
        .max()
        .unwrap_or(0);

    let mut message = String::from("```\nCurrent True Coin balances:\n");
    for MemberBalance { username, balance } in member_balances {
        let _ = writeln!(&mut message, "{username:>width$}: {balance:>4} 🪙");
    }
    message += "```";
    ctx.say(message).await?;

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
    let guild_id = ctx.guild_id().unwrap().get();
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
    let guild_id = ctx.guild_id().unwrap().get();
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
