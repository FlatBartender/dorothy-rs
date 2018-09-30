//! This module contains misc functions, useful sometimes.
//! Anybody can use these functions.
use serenity::builder::*;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::framework::standard::*;

use std::sync::Arc;

use utils::*;
use dorothy::Module;

#[derive(Default)]
pub struct Misc;

impl Module for Misc {
    fn register(framework: StandardFramework) -> StandardFramework {
        framework.group("Misc", |g| {
            g.desc("Miscellaneous commands")
                .cmd("id", MentionIdsCommand::default())
        })
    }
}

#[derive(Default)]
struct MentionIdsCommand;

impl Command for MentionIdsCommand {
    fn options(&self) -> Arc<CommandOptions> {
        let mut options = CommandOptions::default();
        options.desc = Some("gives the ID of roles and users".to_string());
        options.usage = Some("(roles|users)? [@role...] [@user...]".to_string());
        options.min_args = Some(1);
        options.help_available = true;

        Arc::new(options)
    }

    fn execute(&self, _ctx: &mut Context, msg: &Message, args: Args) -> Result<(), CommandError> {
        let mut args = args;
        let mut embed = CreateEmbed::default();
        let subcommand = args.single::<String>()?;

        match subcommand.as_str() {
            "roles" => {
                if msg.mention_roles.is_empty() {
                    return Err("please mention roles when asking for role IDs!".into());
                }

                let roles = role_list_to_mentions(&msg.mention_roles);
                let mut roles = if let Err(e) = roles {
                    return Err(format!("an error has occured while creating the role mentions: {}", e).into());
                } else {
                    roles.unwrap()
                };
                embed = embed.field("Role IDs", roles.pop().unwrap(), false)
                    .fields(roles.iter().map(|s| ("Role IDs (cont)", s, false)));
            },
            "users" => {
                if msg.mentions.is_empty() {
                    return Err("please mention users when asking for user Ids!".into());
                }

                let users = user_list_to_mentions(&msg.mentions);
                let mut users = if let Err(e) = users {
                    return Err(format!("an error has occured while creating the user mentions: {}", e).into());
                } else {
                    users.unwrap()
                };
                embed = embed.field("User IDs", users.pop().unwrap(), false)
                    .fields(users.iter().map(|s| ("User IDs (cont)", s, false)));
            },
            _ => {
                return Err("I don't understand.".into());
            }
        }
        
        msg.channel_id.send_message(|m| {
            m.embed(|_| embed)
        })?;

        Ok(())
    }
}

// Sweet, sweet code duplication....
// @DRY

fn user_list_to_mentions(users: &[User]) -> Result<Vec<String>, String> {
    let mut users = users.iter().map(|user| format!("{} -> {}", user.mention(), user.id.0));
    let users = users.try_fold(FoldStrlenState::new(900), &fold_by_strlen)?.extract();
    Ok(users.iter().map(|v| v.join("\n")).collect())
}

fn role_list_to_mentions(roles: &[RoleId]) -> Result<Vec<String>, String> {
    let mut roles = roles.iter().map(|role| format!("{} -> {}", role.mention(), role.0));
    let roles = roles.try_fold(FoldStrlenState::new(900), &fold_by_strlen)?.extract();
    Ok(roles.iter().map(|v| v.join("\n")).collect())
}
