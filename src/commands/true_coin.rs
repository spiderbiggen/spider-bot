use crate::context::Context;
use db::{UserBalanceConnection, UserBalanceTransaction};
use futures::StreamExt;
use poise::{CreateReply, send_reply};
use serenity::all::User;
use std::collections::BTreeSet;
use std::fmt::Write;
use std::sync::atomic;
use std::sync::atomic::Ordering;

const INITIAL_BALANCE: i64 = 500;

#[expect(clippy::unused_async)]
#[poise::command(
    slash_command,
    guild_only,
    subcommands("balance", "transfer", "leaderboard")
)]
pub(crate) async fn coin(_: Context<'_, '_>) -> Result<(), crate::commands::CommandError> {
    Ok(())
}

#[poise::command(slash_command)]
pub(crate) async fn balance(ctx: Context<'_, '_>) -> Result<(), crate::commands::CommandError> {
    ctx.defer_ephemeral().await?;
    let db = &ctx.data().database;

    let guild_id = ctx.guild_id().unwrap().get();
    let user_id = ctx.author().id.get();
    let message = match db.get_user_balance(guild_id, user_id).await? {
        Some(balance) => format!("You currently have {balance} 🪙"),
        None => {
            db.create_user_balance(guild_id, user_id, INITIAL_BALANCE)
                .await?;
            format!("Welcome to True Coin. You currently have {INITIAL_BALANCE} 🪙")
        }
    };
    ctx.reply(message).await?;

    Ok(())
}

#[poise::command(slash_command)]
pub(crate) async fn transfer(
    ctx: Context<'_, '_>,
    #[description = "Who to send coins to"] user: User,
    #[description = "Amount of coins to send to another user"]
    #[max = 1000]
    amount: u32,
) -> Result<(), crate::commands::CommandError> {
    if user.bot {
        let reply = CreateReply::default()
            .reply(true)
            .ephemeral(true)
            .content("Bot users cannot handle the true power of coins.");
        send_reply(ctx, reply).await?;
        return Ok(());
    }

    let db = &ctx.data().database;

    let from_user = ctx.author();
    let from_user_id = from_user.id.get();
    let guild_id = ctx.guild_id().unwrap().get();
    let result = db
        .transfer_user_balance(guild_id, from_user_id, user.id.get(), i64::from(amount))
        .await;

    let to_name = &user.display_name();
    let from_name = &from_user.display_name();
    let (message, ephemeral) = match result {
        Ok((from_balance, to_balance)) => {
            let width = from_name.len().max(to_name.len());
            let message = format!(
                "```\nSuccessfully transferred {amount} 🪙 to {to_name}. New Balance:\n\
                {from_name:>width$}: {from_balance:>4} 🪙\n\
                {to_name:>width$}: {to_balance:>4} 🪙\n```",
            );
            (message, false)
        }
        Err(db::BalanceTransactionError::SenderUninitialized) => {
            let message = "Use `/coins balance` to initialize your coins.";
            (message.to_string(), true)
        }
        Err(db::BalanceTransactionError::RecipientUninitialized) => {
            let message =
                format!("Tell @{to_name} to use `/coins balance` to initialize their coins.");
            (message, true)
        }
        Err(db::BalanceTransactionError::InsufficientBalance(current_amount)) => {
            let message =
                format!("You do not have enough coins. Current balance {current_amount} 🪙");
            (message, true)
        }
        Err(err) => return Err(err.into()),
    };
    let reply = CreateReply::default()
        .reply(true)
        .ephemeral(ephemeral)
        .content(message);
    send_reply(ctx, reply).await?;
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
    let mut message = String::from("```\nCurrent True Coin balances:\n");
    let mut max_length = atomic::AtomicUsize::new(0);
    let member_balances: BTreeSet<_> = futures::stream::iter(users)
        .filter_map(async |user_balance| {
            let member = guild.member(&ctx, user_balance.user_id).await.ok()?;
            if member.user.bot {
                return None;
            }

            let username = member.display_name().to_string();
            max_length.fetch_max(username.len(), Ordering::Relaxed);
            Some(MemberBalance {
                username,
                balance: user_balance.balance,
            })
        })
        .collect()
        .await;

    let width = *max_length.get_mut();
    member_balances
        .into_iter()
        .rev()
        .for_each(|MemberBalance { username, balance }| {
            writeln!(&mut message, "{username:>width$}: {balance:>4} 🪙").unwrap();
        });
    writeln!(&mut message, "```").unwrap();
    ctx.reply(message).await?;

    Ok(())
}
