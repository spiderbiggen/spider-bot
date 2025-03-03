use crate::context::Context;
use db::{UserBalanceConnection, UserBalanceTransaction};
use poise::{CreateReply, send_reply};
use rand::random_range;
use serenity::all::{User, UserId};
use std::fmt::Write;

const INITIAL_BALANCE: i64 = 100;

#[expect(clippy::unused_async)]
#[poise::command(
    slash_command,
    guild_only,
    subcommands("balance", "transfer", "leaderboard", "bet")
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
        Some(balance) => format!("You currently have {balance} coins."),
        None => {
            db.create_user_balance(guild_id, user_id, INITIAL_BALANCE)
                .await?;
            format!("Welcome to True Coin. You currently have {INITIAL_BALANCE} coins.")
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
            let message = format!(
                "Successfully transferred {amount} coins to {to_name}. New Balance:\n\
                {from_name}: {from_balance}\n\
                {to_name}: {to_balance}",
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
            let message = format!("You do not have enough coins. Current balance {current_amount}");
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

#[poise::command(slash_command)]
pub(crate) async fn leaderboard(ctx: Context<'_, '_>) -> Result<(), crate::commands::CommandError> {
    ctx.defer().await?;
    let db = &ctx.data().database;

    let guild_id = ctx.guild_id().unwrap().get();
    let users = db.get_top_user_balances(guild_id).await?;
    let mut message = String::from("Current True Coin balances:\n");
    for user_balance in users {
        let user_id = UserId::from(user_balance.user_id);
        if let Some(user) = ctx.cache().user(user_id) {
            writeln!(
                &mut message,
                "\t{}: {}",
                user.display_name(),
                user_balance.balance
            )
            .unwrap();
        } else {
            let username = match ctx.http().get_user(user_id).await.ok() {
                Some(user) => user.display_name().to_string(),
                None => user_id.to_string(),
            };
            writeln!(&mut message, "\t{}: {}", username, user_balance.balance).unwrap();
        };
    }
    ctx.reply(message).await?;

    Ok(())
}

#[expect(clippy::unused_async)]
#[poise::command(slash_command, subcommands("poker_chip"))]
pub(crate) async fn bet(_: Context<'_, '_>) -> Result<(), crate::commands::CommandError> {
    Ok(())
}

/// Bet some of you coins for a chance of double your bet.
/// A die roll determines the outcome.
///
/// 1-3 receive 1 coin\
/// 4-6 receive double your bet.\
#[poise::command(slash_command)]
pub(crate) async fn poker_chip(
    ctx: Context<'_, '_>,
    #[description = "Amount of coins to stake on this bet"]
    #[min = 2]
    bet: u32,
) -> Result<(), crate::commands::CommandError> {
    ctx.defer().await?;
    let db = &ctx.data().database;

    let guild_id = ctx.guild_id().unwrap().get();
    let user_id = ctx.author().id.get();
    let Some(balance) = db.get_user_balance(guild_id, user_id).await? else {
        let reply = CreateReply::default()
            .reply(true)
            .ephemeral(true)
            .content("Use `/coins balance` to initialize your coins.");
        send_reply(ctx, reply).await?;
        return Ok(());
    };
    let bet = i64::from(bet);
    if balance < bet {
        let message = format!("You do not have enough coins. Current balance {balance}");
        let reply = CreateReply::default()
            .reply(true)
            .ephemeral(true)
            .content(message);
        send_reply(ctx, reply).await?;
        return Ok(());
    }

    let roll = random_range(1..=6);
    let reward = match roll {
        1..=3 => 1i64,
        _ => bet * 2,
    };

    let change = reward - bet;
    let new_balance = db.add_user_balance(guild_id, user_id, change).await?;

    let gain_msg = if change.is_positive() {
        "receive"
    } else {
        "lose"
    };
    let message = format!(
        "You staked {bet} and rolled a {roll}. You {gain_msg} {} coins. New Balance: {new_balance}",
        change.abs()
    );

    ctx.reply(message).await?;
    Ok(())
}
