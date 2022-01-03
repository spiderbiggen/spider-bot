use rand::distributions::Uniform;
use rand::{thread_rng, Rng};
use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::channel::Message;

#[command]
#[delimiters("d", " ")]
#[min_args(1)]
#[max_args(2)]
pub async fn roll(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let dice: u16;
    let count: u16;

    let first = args.single::<u16>()?;
    match args.single::<u16>() {
        Ok(die) => {
            dice = die;
            count = first;
        }
        Err(..) => {
            dice = first;
            count = 1;
        }
    }

    let message = thread_rng()
        .sample_iter(Uniform::new_inclusive(1, dice as usize))
        .take(count as usize)
        .map(|s| s.to_string())
        .collect::<Vec<String>>()
        .join(", ");

    msg.reply(ctx, message).await?;

    Ok(())
}
