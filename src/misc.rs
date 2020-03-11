//! This module contains misc functions, useful sometimes.
//! Anybody can use these functions.
use serenity::framework::standard::*;
use serenity::model::prelude::*;
use serenity::prelude::*;

use serenity::framework::standard::macros::{command, group};

use utils::*;

#[group]
pub struct Misc;

#[command]
#[owners_only]
#[usage("<channel id> <message>")]
#[description("Sends a message in the supplied channel")]
#[num_args(2)]
#[only_in("dms")]
fn say(ctx: &mut Context, _msg: &Message, args: Args) -> Result<(), CommandError> {
    let mut args = args;
    let channel = args.single::<ChannelId>()?;

    channel.say(&ctx.http, args.rest())?;

    Ok(())
}

#[command]
#[usage("(roles|users)? [@role...] [@user...]")]
#[description("Gives the ID of roles and users")]
#[min_args(1)]
#[required_permissions("MANAGE_GUILD")]
fn id(ctx: &mut Context, msg: &Message, args: Args) -> Result<(), CommandError> {
    let mut args = args;
    let subcommand = args.single::<String>()?;

    let mut mentions = match subcommand.as_str() {
        "roles" => {
            if msg.mention_roles.is_empty() {
                return Err("please mention roles when asking for role IDs!".into());
            }

            role_list_to_mentions(&msg.mention_roles).map_err(CommandError)?
        }
        "users" => {
            if msg.mentions.is_empty() {
                return Err("please mention users when asking for user Ids!".into());
            }

            user_list_to_mentions(&msg.mentions).map_err(CommandError)?
        }
        _ => {
            return Err("I don't understand.".into());
        }
    };

    msg.channel_id.send_message(&ctx.http, |m| {
        m.embed(|e| {
            e.field("IDs", mentions.pop().unwrap(), false);
            e.fields(mentions.iter().map(|s| ("IDs (cont)", s, false)));
            e
        })
    })?;

    Ok(())
}

// Sweet, sweet code duplication....
// @DRY

fn user_list_to_mentions(users: &[User]) -> Result<Vec<String>, String> {
    let mut users = users
        .iter()
        .map(|user| format!("{} -> {}", user.mention(), user.id.0));
    let users = users
        .try_fold(FoldStrlenState::new(900), &fold_by_strlen)?
        .extract();
    Ok(users.iter().map(|v| v.join("\n")).collect())
}

fn role_list_to_mentions(roles: &[RoleId]) -> Result<Vec<String>, String> {
    let mut roles = roles
        .iter()
        .map(|role| format!("{} -> {}", role.mention(), role.0));
    let roles = roles
        .try_fold(FoldStrlenState::new(900), &fold_by_strlen)?
        .extract();
    Ok(roles.iter().map(|v| v.join("\n")).collect())
}
