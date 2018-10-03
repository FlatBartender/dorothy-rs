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
pub struct GetCommand;
#[derive(Default)]
pub struct CreateCommand;
#[derive(Default)]
pub struct SetCommand;
#[derive(Default)]
pub struct AddRolesCommand;
#[derive(Default)]
pub struct AddGameCommand;
#[derive(Default)]
pub struct CommitCommand;

lazy_static! {
    static ref INCOMPLETE_SERVERS: RwLock<HashMap<GuildId, Server>> = {
        // @TUNE Change the number of reserved slots
        RwLock::new(HashMap::with_capacity(500))
    };
}

// @DRY The code here has a lot of redundancies. Better clean up someday.

/// Gets a server from the loaded config or a default value, then prints the data in an embed.
impl Command for GetCommand {
    fn options(&self) -> Arc<CommandOptions> {
        let mut options = CommandOptions::default();
        options.desc = Some("Gets the configuration for this server if it's already loaded.".to_string());
        options.help_available = true;
        options.max_args = Some(0);

        Arc::new(options)
    }

    fn execute(&self, _ctx: &mut Context, msg: &Message, _args: Args) -> Result<(), CommandError> {
        // Unwrap is safe here because this command is only available in servers.
        let server_id = msg.guild_id.unwrap();

        let config = CONFIG.read().expect("couldn't lock CONFIG for reading");
        let server = config.get(&server_id);

        let server = match server {
            // Server config exists, put it in the INCOMPLETE_SERVERS map.
            Some(s) => s.clone(),
            None    => return Err(CommandError("Couldn't find server in config file".to_string())),
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
}

/// Creates server data without roles, and with an empty Games vec.
impl Command for CreateCommand {
    fn options(&self) -> Arc<CommandOptions> {
        let mut options = CommandOptions::default();
        options.desc = Some("Creates a new configuration. the start and end expressions are cron syntax, but with an additionnal field on the left for seconds.".to_string());
        options.usage = Some("<channel_id> <start exp> <end exp>".to_string());
        options.help_available = true;
        options.max_args = Some(3);
        options.min_args = Some(3);

        Arc::new(options)
    }

    fn execute(&self, _ctx: &mut Context, msg: &Message, args: Args) -> Result<(), CommandError> {
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
}

/// Sets the values of the server if it's loaded in the INCOMPLETE_SERVERS list
impl Command for SetCommand {
    fn options(&self) -> Arc<CommandOptions> {
        let mut options = CommandOptions::default();
        options.desc = Some("Sets the values of the server if it's currently being edited (eg, after create or get). Start and end expressions are cron syntax but with an additionnal field on the left for seconds.".to_string());
        options.usage = Some("<channel_id> <start exp> <end exp>".to_string());
        options.help_available = true;
        options.max_args = Some(3);
        options.min_args = Some(3);

        Arc::new(options)
    }

    fn execute(&self, _ctx: &mut Context, msg: &Message, args: Args) -> Result<(), CommandError> {
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
}

/// Adds roles to the server, to be @ed on the start message
impl Command for AddRolesCommand {
    fn options(&self) -> Arc<CommandOptions> {
        let mut options = CommandOptions::default();
        options.desc = Some("Adds roles to be @ed when the start message is sent.".to_string());
        options.usage = Some("role_id [role_id, role_id, ...]".to_string());
        options.help_available = true;
        options.min_args = Some(1);

        Arc::new(options)
    }

    fn execute(&self, _ctx: &mut Context, msg: &Message, args: Args) -> Result<(), CommandError> {
        let mut args = args;

        // Unwrap is safe here because this command is only available in servers.
        let server_id = msg.guild_id.unwrap();

        let mut roles = args.iter::<u64>().filter_map(|a| a.ok()).map(RoleId).collect();

        {
            let mut incomplete_servers = INCOMPLETE_SERVERS.write().expect("couldn't lock INCOMPLETE_SERVERS for writing");
            let server = incomplete_servers.entry(server_id).or_default();
            match server.role_ids {
                None => server.role_ids = Some(roles),
                Some(ref mut s) => s.append(&mut roles)
            }
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
}

/// Adds a game to the server
impl Command for AddGameCommand {
    fn options(&self) -> Arc<CommandOptions> {
        let mut options = CommandOptions::default();
        options.desc = Some("Adds a game to the server configuration.".to_string());
        options.usage = Some("<name> <emoji> <channel_id> [role_id, role_id, ...]".to_string());
        options.help_available = true;
        options.min_args = Some(3);

        Arc::new(options)
    }

    fn execute(&self, _ctx: &mut Context, msg: &Message, args: Args) -> Result<(), CommandError> {
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
}

/// Saves the incomplete server to the real config list and puts it on the disk.
impl Command for CommitCommand {
    fn options(&self) -> Arc<CommandOptions> {
        let mut options = CommandOptions::default();
        options.desc = Some("Adds the configuration made to the live configuration and saves it to disk. Ask an owner to rehash the configuration.".to_string());
        options.help_available = true;

        Arc::new(options)
    }

    fn execute(&self, _ctx: &mut Context, msg: &Message, _args: Args) -> Result<(), CommandError> {
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
}

#[inline]
/// Useful function for displaying a server config.
/// BE CAREFUL: if there are too many roles, it will create an invalid message!
/// @TODO use the fold_by_strlen function
/// @DRY there's some code dupe here
fn display_server(server: &Server) -> CreateEmbed {
    CreateEmbed::default()
        .field("Channel", server.channel_id.mention(), false)
        .field("Event times", format!("Starts: {}\n  Ends: {}\n", &server.start, &server.end), true)

        .field("Roles", match server.role_ids {
               None => "None".to_string(),
               Some(ref roles) => roles.iter().map(|r| r.mention()).collect::<Vec<String>>().join(", "),
        }, false)
        .fields(server.games.iter().map(|g| {
            (format!("{} {}", g.emoji, g.name), 
             format!("In {}, {}", g.channel_id.mention(), match g.role_ids {
                 None => "None".to_string(),
                 Some(ref roles) => roles.iter().map(|r| r.mention()).collect::<Vec<String>>().join(", "),
             }), false)
        }))
}
