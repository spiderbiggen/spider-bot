use crate::context::Context;
use db::{UserBalanceConnection, UserBalanceTransaction};
use poise::{CreateReply, send_reply};
use serenity::all::User;

const INITIAL_BALANCE: i64 = 100;

#[expect(clippy::unused_async)]
#[poise::command(slash_command, subcommands("balance", "transfer"))]
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
    #[description = "Amount of coins to send to another user"] amount: u32,
) -> Result<(), crate::commands::CommandError> {
    let db = &ctx.data().database;

    let from_user = ctx.author();
    let from_user_id = from_user.id.get();
    let guild_id = ctx.guild_id().unwrap().get();
    let result = db
        .transfer_user_balance(guild_id, from_user_id, user.id.get(), i64::from(amount))
        .await;
    let to_name = &user.name;
    let from_name = &from_user.name;
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
