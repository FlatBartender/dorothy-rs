use serenity::{
    framework::standard::{
        help_commands::with_embeds,
        macros::help,
        Args, CommandError, CommandGroup, CommandResult,
        DispatchError::{self, NotEnoughArguments, Ratelimited, TooManyArguments},
        HelpOptions,
    },
    model::prelude::*,
    prelude::*,
};

use std::collections::HashSet;

#[derive(Default)]
pub struct Dorothy;

impl EventHandler for Dorothy {
    fn ready(&self, _ctx: Context, _data: Ready) {
        info!("NOBODY EXPECTS THE DOROTHINQUISITION!!!");
    }
}

pub fn print_command_used(_ctx: &mut Context, msg: &Message, cmd_name: &str) -> bool {
    info!(
        "{} ({}) used {} in server {:?}, channel {:?}",
        msg.author.name, msg.author.id, cmd_name, msg.guild_id, msg.channel_id
    );

    true
}

pub fn command_error_logger(
    ctx: &mut Context,
    msg: &Message,
    cmd_name: &str,
    result: Result<(), CommandError>,
) {
    if let Err(e) = result {
        warn!("Command error for command {}: {:?}", cmd_name, e);
        if let Err(e) = msg
            .channel_id
            .say(&ctx.http, format!("An error has occurred: `{:#?}`", e))
        {
            warn!(
                "An error has occurred while sending the error message (lol): {:?}",
                e
            );
        }
    }
}

pub fn dispatch_error_handler(ctx: &mut Context, msg: &Message, err: DispatchError) {
    warn!("A dispatch error was encountered: {:?}", err);
    match err {
        NotEnoughArguments { min, given } => msg.channel_id.say(
            &ctx.http,
            format!("Need {} arguments, but only got {}.", min, given),
        ),
        TooManyArguments { max, given } => msg.channel_id.say(
            &ctx.http,
            format!("Max arguments allowed is {}, but got {}.", max, given),
        ),
        Ratelimited(seconds) => msg.channel_id.say(
            &ctx.http,
            format!("Too fast ! Try again in {} seconds.", seconds),
        ),
        _ => msg
            .channel_id
            .say(&ctx.http, format!("Unhandled dispatch error.")),
    }
    .expect("Fatal error");
}

pub fn normal_message(ctx: &mut Context, msg: &Message) {
    info!("{} ({}) in {}: {}", msg.author.tag(), msg.author.id, msg.channel_id, msg.content);
    if !msg.attachments.is_empty() {
        for attachment in msg.attachments.iter() {
            info!("\t{}", attachment.proxy_url);
        }
    }
}

#[help]
pub fn embed_help(
    context: &mut Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    with_embeds(context, msg, args, &help_options, groups, owners)
}
