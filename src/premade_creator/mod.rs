//! This module is a bit different from others.
//! Since it operates not based on messages but on time events, it doesn't register a struct in
//! the handlers array but instead the state is kept in a static variable.
//!
//! It needs to find the `premade-creator.start` and the `premade-creator.end` strings in the
//! config, syntax is cron (see job_scheduler crate documentation).
//! At the start event it will @everyone with a list of games associated with emojis 
//! under `premade-creator.games` (as key-value pairs, game name -> [emoji*, team size]).
//! At the end event, it will look for reactions on the message posted for the start, form teams
//! with people who reacted to the corresponding emoji, send multiple messages to the teams, and @
//! the remaining people who haven't been chosen.
//! The job scheduler will check every `premade-creator.tick` seconds (int) for the events.
//! `premade-creator.servers` will be a table associating a server ID in a string (can't use u64s
//! as keys) with an array containing, first, the channel id, then role ids. For each of these 
//! servers, the bot will send the message on the specified channel, while @ing the specified role
//! (or nobody if there's no role).
//!
//! * an emoji is either a string (for unicode emojis) or an array [name, id].
//!
//!
//! Configuration example:
//! ```toml
//! [premade-creator]
//! start = "0  * * * * *"
//! end   = "30 * * * * *"
//! tick  = 1
//! games = [
//!     ["légoléjande",   "🦈", "5"],
//!     ["overwatch",     "🔫", "6"],
//!     ["rocket league", "🏎", "3"],
//! ]
//! 
//! [premade-creator.servers]
//! "server id"   = [channel id, role id 1, role id 2]
//! "server id 2" = [channel id 2]
//! ```

use job_scheduler::{JobScheduler, Job};

use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::framework::StandardFramework;
use serenity::utils::Colour;
use serenity::builder::*;

use std::thread;
use std::time::Duration;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

use get_settings;
use utils::*;
use dorothy::Module;

lazy_static! {
    static ref STATE: Arc<RwLock<HashMap<ChannelId, MessageId>>> = {
        // @TUNE Change the number of reserved slots
        // Keeping the space for 500 message IDs is really inexpensive memory-wise (a few
        // kilobytes, maybe a couple dozen kb with the hashmap overhead AT MOST).
        Arc::new(RwLock::new(HashMap::with_capacity(500)))
    };
}

/// Convenience method to get the games list.
///
/// Currently using the global settings, but in the future we might want to get things from a data
/// store.
fn get_games() -> Vec<GameInfo> {
    let settings = get_settings();
    let settings = settings.read().expect("couldn't acquire read lock on settings");

    let games = settings.get_array("premade-creator.games").expect("couldn't get games");

    games.into_iter().map(|v| {
        let v = v.into_array().expect("value isn't an array");
        let name = v[0].clone().into_str().expect("couldn't parse name");

        // @TODO Get infos on how ser/de is done. We might avoid this little hell.
        let emoji = match v[1].clone().into_str() {
            // Emoji is a unicode emoji
            Ok(s) => ReactionType::Unicode(s.to_string()),
            // Emoji is either unparseable or a custom emoji
            Err(_) => {
                panic!("Can't use custom emojis yet");
            }
        };

            let team_size: u32 = v[2].clone().into_str().expect("couldn't deserialize team size").parse().expect("couldn't parse team size");

            GameInfo {
                name,
                emoji,
                team_size,
            }
    }).collect()
}

/// Convenience method to get the servers list.
///
/// Currently using the global settings, but in the future we might want to get things from a data
/// store.
fn get_servers() -> Vec<Server> {
    let settings = get_settings();
    let settings = settings.read().expect("couldn't acquire read lock on settings");
    
    let servers = settings.get_table("premade-creator.servers").expect("couldn't get servers");
    servers.into_iter().map(|(server, infos)| {
        let server_id = GuildId(server.parse().expect("couldn't parse server string"));
        let infos = infos.into_array().expect("couldn't parse server infos");
        if infos.len() == 0 {
            panic!("server infos need to have at least one element");
        }
        let channel_id = ChannelId(infos[0].clone().try_into().expect("couldn't deserialize channel"));

        let role_ids = if infos[1..].len() == 0 {
            None
        } else {
            Some(infos[1..].iter().map(|role| {
                let role = role.clone().try_into().expect("couldn't parse role id");
                RoleId(role)
            }).collect())
        };
    
        Server {
            server_id,
            channel_id,
            role_ids
        }
    }).collect()
}

#[derive(Default)]
pub struct PremadeCreator;

impl Module for PremadeCreator {
    fn register(framework: StandardFramework) -> StandardFramework {
        let (start_time, end_time) = {
            let settings = get_settings();
            let settings = settings.read().expect("couldn't acquire read lock on settings");

            let start = settings.get_str("premade-creator.start").expect("couldn't get cron line for start event");
            let end   = settings.get_str("premade-creator.end").expect("couldn't get cron line for end event");

            (start, end)
        };

        thread::spawn(move || {
            let mut sched = JobScheduler::new();
            sched.add(Job::new(start_time.parse().expect("bad start syntax"), &process_start));
            sched.add(Job::new(  end_time.parse().expect("bad start syntax"), &process_end));

            loop {
                sched.tick();
                let tick_size = {
                    let settings = get_settings();
                    let settings = settings.read().expect("couldn't lock settings for reading");
                    Duration::from_secs(settings.get::<u64>("premade-creator.tick").expect("couldn't find tick length"))
                };
                thread::sleep(tick_size);
            }
        });

        framework
    }
}

/// Convenience function to turn an Option<Vec<RoleId>> into a string mentioning the roles, if not
/// None.
fn role_list_to_mentions(roles: &Option<Vec<RoleId>>) -> String {
    match roles {
        // If there's no role configured, don't @ anyone
        None => "".to_string(),
        // If there's any role configured, mention all of them
        Some(ref ids) => {
            ids.iter().map(&RoleId::mention).collect::<Vec<String>>().join(", ")
        }
    }
}

/// Function called at the "start" event, which will go through all the servers in the list to @
/// the proper roles proposing them a few games. Potential players need to react with the proper
/// reactions.
fn process_start() {
    info!("Starting the premade creation process...");

    // Yeah I realize I could use the r#""# notation but this is way more readable imo.
    let embed_description = vec![
        "Today, these following games are available!".to_string(),
        "React with the corresponding emoji to participate!\n".to_string(),
    ].join("\n");

    let games = get_games();
    let mut reactions   = Vec::with_capacity(games.len());
    let mut embed_games = Vec::with_capacity(games.len());
    for g in games.iter() {
        reactions.push(g.emoji.clone());
        embed_games.push(format!("{} -> `{}`", g.emoji, g.name));
    }

    let embed_games = embed_games.into_iter().try_fold(FoldStrlenState::new(900), &fold_by_strlen).expect("error while creating games message");
    let embed_games = embed_games.extract().iter().map(|v| v.join("\n")).collect::<Vec<String>>();

    if embed_games.len() == 0 {
        return;
    }

    let embed = CreateEmbed::default();
    let embed = embed.color(Colour::from_rgb(120, 17, 176))
                 .title("Pick your games!")
                 .description(&embed_description)
                 .field("Games", &embed_games[0], false)
                 .fields(embed_games[1..].iter().map(|g| ("Games (cont)", g, false)));

    let message = CreateMessage::default();
    let message = message.embed(|_| embed)
        .reactions(reactions.into_iter());
    
    let servers = get_servers();

    let state = STATE.clone();
    let mut state = state.write().expect("couldn't lock state");
    for s in servers.iter() {
        let message = message.clone();
        let message = message.content(role_list_to_mentions(&s.role_ids));
        match s.channel_id.send_message(|_| message.clone()) {
            // Message successfully sent, keep the ID in memory
            Ok(msg) => {
                state.insert(s.channel_id, msg.id);
            },
            // Message wasn't sent correctly. Forwarding error to user.
            Err(e) => {
                warn!("Couldn't send message to server {}: {:?}", s.server_id.0, e);
            }
        };
    }
}

/// Function called at the "end" event. Traverses all the list of servers to find out the messages
/// it sent, and writes a message with all players for every particular game.
fn process_end() {
    info!("Ending the premade creation process...");

    let embed = CreateEmbed::default();
    let embed = embed.color(Colour::from_rgb(120, 17, 176))
                 .title("Today's players")
                 .description("The following players want to play:");

    let games = get_games();
    let servers = get_servers();

    for s in servers.iter() {
        let message_id = {
            let state = STATE.clone();
            let state = state.read().expect("couldn't lock settings for reading");
            match state.get(&s.channel_id) {
                None => {
                    warn!("Initial message not found for server {}!", s.server_id);
                    continue;
                },
                Some(mid) => mid.clone(),
            }
        };

        let mut embed = embed.clone();
        for g in games.iter() {
            let mentions = s.channel_id.reaction_users(message_id, g.emoji.clone(), None, None).expect("couldn't get emojis");
            let mentions = mentions.iter().filter(|user| !user.bot);
            let mentions = mentions.map(&User::mention).try_fold(FoldStrlenState::new(900), &fold_by_strlen).expect("error while making mentions");
            let mut mentions = mentions.extract().iter().map(|v| v.join(", ")).collect::<Vec<String>>();
            
            // If nobody answered for this particular game, skip
            if mentions.len() == 0 {
                continue;
            }

            embed = embed.field(format!("{} {}", g.emoji, g.name), mentions[0].clone(), false)
                .fields(mentions[1..].iter().map(|m| (format!("{} {} (cont)", g.emoji, g.name), m, false)));

        }
        
        let result = s.channel_id.send_message(|m| {
            let message = m.embed(|_| embed);
            message.content(role_list_to_mentions(&s.role_ids))
        });
        if let Err(err) = result {
            warn!("The message couldn't be sent to server {}: {:?}", s.server_id.0, err);
        }
        let state = STATE.clone();
        let mut state = state.write().expect("couldn't lock state for writing");
        state.remove(&s.channel_id);
    }
}

/// Represents a game info.
/// A game has a name that will represent it everywhere, an emoji used in reactions, and a
/// team_size (currently not used, but in the future we might want to make random teams from all
/// the answers).
#[derive(Clone)]
struct GameInfo {
    name: String,
    emoji: ReactionType,
    team_size: u32,
}

/// Represents a server.
/// A server has a server id, a channel id representing the channel to which the messages will be
/// sent, and an optional list of roles to be @ed when the messages are sent.
#[derive(Clone)]
struct Server {
    server_id: GuildId,
    channel_id: ChannelId,
    role_ids: Option<Vec<RoleId>>,
}
