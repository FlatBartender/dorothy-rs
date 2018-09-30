use cron;

use serenity::builder::*;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::framework::standard::*;
use serenity::utils::*;
use serenity::model::misc::Mentionable;

use std::sync::Arc;
use std::collections::HashMap;
use std::sync::RwLock;

use super::Server;
use super::CONFIG;

#[derive(Default)]
pub struct CreatorCommand;

lazy_static! {
    static ref INCOMPLETE_SERVERS: RwLock<HashMap<GuildId, Server>> = {
        // @TUNE Change the number of reserved slots
        RwLock::new(HashMap::with_capacity(500))
    };
}

impl Command for CreatorCommand {
    // @TODO be more specific in desc and usage messages
    fn options(&self) -> Arc<CommandOptions> {
        let mut options = CommandOptions::default();
        options.desc = Some("helps configuring the Premade Creator".to_string());
        options.usage = Some("pls update me lol".to_string());
        options.required_permissions = Permissions::MANAGE_GUILD;
        options.help_available = true;
        options.min_args = Some(1);

        Arc::new(options)
    }

    fn execute(&self, _ctx: &mut Context, msg: &Message, args: Args) -> Result<(), CommandError> {
        let mut args = args;
        let subcommand = args.single::<String>()?;

        match subcommand.as_str() {
            "get"    => get_incomplete_server(msg, args),
            "create" => create_incomplete_server(msg, args),
            "set"    => set_incomplete_server(msg, args),
            "add" => {
                let add_type = args.single::<String>()?;
                match add_type.as_str() {
                    "role" | "roles" => add_roles_incomplete_server(msg, args),
                    "game"           => add_game_incomplete_server(msg, args),
                    _ => Err(CommandError(format!("invalid parameter: {}", add_type))),
                }
            },
            "commit" => commit_incomplete_server(msg, args),
            _ => Err(CommandError(format!("unknown parameter: {}", subcommand))),
        }
    }
}

// @DRY The code here has a lot of redundancies. Better clean up someday.

#[inline]
/// Useful function for displaying a server config.
/// BE CAREFUL: if there are too many roles, it will create an invalid message!
/// @TODO use the fold_by_strlen function
/// @DRY there's some code dupe here
fn display_server(server: &Server) -> CreateEmbed {
    CreateEmbed::default()
        .field("Channel", format!("{} ({})", &server.channel_id, &server.channel_id.0), false)
        .field("Event times", format!("Starts: {}\n  Ends: {}\n", &server.start, &server.end), true)

        .field("Roles", match server.role_ids {
               None => "None".to_string(),
               Some(ref roles) => roles.iter().map(|r| r.mention()).collect::<Vec<String>>().join(", "),
        }, false)
        .fields(server.games.iter().map(|g| {
            (format!("{} {}", g.emoji, g.name), 
             format!("In {}, {}", g.channel_id, match g.role_ids {
                 None => "None".to_string(),
                 Some(ref roles) => roles.iter().map(|r| r.mention()).collect::<Vec<String>>().join(", "),
             }), false)
        }))
}

/// Gets a server from the loaded config or a default value, then prints the data in an embed.
fn get_incomplete_server(msg: &Message, _args: Args) -> Result<(), CommandError> {
    // Unwrap is safe here because this command is only available in servers.
    let server_id = msg.guild_id.unwrap();
    
    let config = CONFIG.read().expect("couldn't lock CONFIG for reading");
    let server = config.get(&server_id);
    
    let server = match server {
        // Server config exists, put it in the INCOMPLETE_SERVERS map.
        Some(s) => s.clone(),
        None    => Server::default(),
    };
    
    msg.channel_id.send_message(|m| m.embed(|_| display_server(&server)
                                            .title("Server configuration loaded!")
                                            .color(Colour::from_rgb(120, 17, 176))
                                            )
                                )?;

    let mut incomplete_servers = INCOMPLETE_SERVERS.write().expect("couldn't lock INCOMPLETE_SERVERS for writing");
    incomplete_servers.insert(server_id, server);

    Ok(())
}

/// Creates server data without roles, and with an empty Games vec.
fn create_incomplete_server(msg: &Message, args: Args) -> Result<(), CommandError> {
    let mut args = args;
    // Unwrap is safe here because this command is only available in servers.
    let server_id = msg.guild_id.unwrap();
    let mut server = Server::default();
    
    if args.remaining() < 3 {
        return Err(CommandError("The Create command takes 3 arguments".to_string()));
    }
    
    server.channel_id = ChannelId(args.single()?);
    
    // @IDEA Maybe change the Server type to use Schedules instead of Strings for start and end
    // once this part of the module works
    // Get the two strings that we will use in the server
    let start: String = args.single_quoted()?;
    let   end: String = args.single_quoted()?;
    // Check if they are valid syntax
    let _t: cron::Schedule = start.parse()?;
    let _t: cron::Schedule =   end.parse()?;
    
    // If we're here then everything is valid.
    server.start = start;
    server.end   = end;

    msg.channel_id.send_message(|m| m.embed(|_| display_server(&server)
                                            .title("New configuration created!")
                                            .color(Colour::from_rgb(120, 17, 176))
                                            )
                                )?;

    let mut incomplete_servers = INCOMPLETE_SERVERS.write().expect("couldn't lock INCOMPLETE_SERVERS for writing");
    incomplete_servers.insert(server_id, server);

    Ok(())
}

fn set_incomplete_server(msg: &Message, args: Args) -> Result<(), CommandError> {
    let mut args = args;

    // Unwrap is safe here because this command is only available in servers.
    let server_id = msg.guild_id.unwrap();
    
    if args.remaining() < 3 {
        return Err(CommandError("The Create command takes 3 arguments".to_string()));
    }
    
        
    let channel_id = ChannelId(args.single()?);
    
    // @IDEA Maybe change the Server type to use Schedules instead of Strings for start and end
    // once this part of the module works
    // Get the two strings that we will use in the server
    let start: String = args.single_quoted()?;
    let   end: String = args.single_quoted()?;
    // Check if they are valid syntax
    let _t: cron::Schedule = start.parse()?;
    let _t: cron::Schedule =   end.parse()?;
    
    // If we're here then everything is valid.
    {
        let mut incomplete_servers = INCOMPLETE_SERVERS.write().expect("couldn't lock INCOMPLETE_SERVERS for writing");
        let server = incomplete_servers.entry(server_id).or_default();
        server.channel_id = channel_id;
        server.start = start;
        server.end   = end;
    }
    
    // We're relocking the INCOMPLETE_SERVERS here to keep the writing section as small as
    // possible.
    let incomplete_servers = INCOMPLETE_SERVERS.read().expect("couldn't lock INCOMPLETE_SERVERS for reading");
    // Once we get here there *is* a server in the map so it's safe to unwrap.
    let server = incomplete_servers.get(&server_id).unwrap();
    msg.channel_id.send_message(|m| m.embed(|_| display_server(&server)
                                            .title("Server configuration modified (don't forget to commit it)")
                                            .color(Colour::from_rgb(120, 17, 176))
                                            )
                                )?;

    Ok(())
}

fn add_roles_incomplete_server(msg: &Message, args: Args) -> Result<(), CommandError> {
    let mut args = args;
    
    // Unwrap is safe here because this command is only available in servers.
    let server_id = msg.guild_id.unwrap();

    let roles = args.iter::<u64>().filter_map(|a| a.ok()).map(RoleId).collect();

    {
        let mut incomplete_servers = INCOMPLETE_SERVERS.write().expect("couldn't lock INCOMPLETE_SERVERS for writing");
        let server = incomplete_servers.entry(server_id).or_default();
        server.role_ids = Some(roles);
    }

    // We're relocking the INCOMPLETE_SERVERS here to keep the writing section as small as
    // possible.
    let incomplete_servers = INCOMPLETE_SERVERS.read().expect("couldn't lock INCOMPLETE_SERVERS for reading");
    // Once we get here there *is* a server in the map so it's safe to unwrap.
    let server = incomplete_servers.get(&server_id).unwrap();
    msg.channel_id.send_message(|m| m.embed(|_| display_server(&server)
                                            .title("Roles set (don't forget to commit)")
                                            .color(Colour::from_rgb(120, 17, 176))
                                            )
                                )?;

    Ok(())
}

fn add_game_incomplete_server(msg: &Message, args: Args) -> Result<(), CommandError> {
    let mut args = args;

    // Unwrap is safe here because this command is only available in servers.
    let server_id = msg.guild_id.unwrap();
    
    let name        = args.single_quoted()?;
    let emoji       = args.single()?;
    let channel_id  = args.single()?;
    let role_ids: Vec<RoleId> = args.iter::<u64>().filter_map(|a| a.ok()).map(RoleId).collect();
    let role_ids = if role_ids.is_empty() { None } else { Some(role_ids) };
    
    let game = super::GameInfo {
        name,
        emoji,
        role_ids,
        channel_id,
    };
    
    {
        let mut incomplete_servers = INCOMPLETE_SERVERS.write().expect("couldn't lock INCOMPLETE_SERVERS for writing");
        let server = incomplete_servers.entry(server_id).or_default();
        server.games.push(game);
    }

    // We're relocking the INCOMPLETE_SERVERS here to keep the writing section as small as
    // possible.
    let incomplete_servers = INCOMPLETE_SERVERS.read().expect("couldn't lock INCOMPLETE_SERVERS for reading");
    // Once we get here there *is* a server in the map so it's safe to unwrap.
    let server = incomplete_servers.get(&server_id).unwrap();
    msg.channel_id.send_message(|m| m.embed(|_| display_server(&server)
                                            .title("Game added (don't forget to commit)")
                                            .color(Colour::from_rgb(120, 17, 176))
                                            )
                                )?;

    Ok(())
}


fn commit_incomplete_server(msg: &Message, _args: Args) -> Result<(), CommandError> {
    // Unwrap is safe here because this command is only available in servers.
    let server_id = msg.guild_id.unwrap();
    let incomplete_servers = INCOMPLETE_SERVERS.read().expect("couldn't lock INCOMPLETE_SERVERS for reading");
    // Once we get here there *is* a server in the map so it's safe to unwrap.
    let server = incomplete_servers.get(&server_id).ok_or("Server config not found in INCOMPLETE_SERVERS")?;

    {
        let mut config = CONFIG.write().expect("couldn't lock CONFIG for writing");
        config.insert(server_id, server.clone());
    }

    super::save_config()?;

    msg.channel_id.send_message(|m| m.content("Configuration saved and written to disk."))?;

    Ok(())
}


