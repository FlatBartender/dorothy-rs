//! This module is a bit different from others.
//! Since it operates not based on messages but on time events, it doesn't register a struct in
//! the handlers array but instead the state is kept in a static variable.
//!
//! At the start event it will mention specific roles with a list of games associated with emojis
//! under `premade-creator.games` (as key-value pairs, game name -> [emoji*, team size]).
//! At the end event, it will look for reactions on the message posted for the start, and send a
//! message with all the people who reacted, and mention the specific roles.
//! The job scheduler will check every `premade-creator.tick` seconds (int) for the events.
//! "specific roles" are stored individually for each server. If no role is specified, no mention
//! gets sent.
//! * an emoji is either a string (for unicode emojis) or an array [name, id].
//!
//! Configuration example:
//! ```json
//! {
//!     "359818298067779584": {                 // Server ID, as a string
//!         "channel_id": 376355712223412225,   // Channel ID, as a number
//!         "start": "0  * * * * *",            // Like cron, but with an additional number for
//!                                             // seconds (here, at second 0 of every minute)
//!         "end":   "30 * * * * *",            // Same
//!         "games": [{
//!             "name": "légoléjande",              // Game name
//!             "channel_id": 491722712562139136,   // Where to send the message to
//!             "emoji": {"name": "🦈"},            // Emoji. BE CAREFUL, the Unicode variant still
//!                                                 // asks for the "name" field!
//!             "role_ids": [491723066372653057]    // An optional list of roles to mention
//!         }, {
//!             "name": "overwatch",
//!             "channel_id": 491722745500008458,
//!             "emoji": {"name": "🔫"}
//!         }, {
//!             "name": "Rocket League",
//!             "channel_id": 491722776055644160,
//!             "emoji": {"name": "🏎"}
//!         }],
//!         "role_ids": [
//!             376685245409525760              // List of roles in number form
//!         ]
//!     }
//! }
//! ```

use job_scheduler::{Job, JobScheduler};

use serenity::builder::*;
use serenity::framework::standard::{Args, CommandError};
use serenity::framework::StandardFramework;
use serenity::model::permissions::Permissions;
use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::utils::Colour;

use serde_json::{from_reader, to_writer_pretty};

use std::collections::HashMap;
use std::fs::File;
use std::sync::mpsc::*;
use std::sync::RwLock;
use std::thread;
use std::time::Duration;

use dorothy::Module;
use utils::*;
use SETTINGS;

mod creator_command;

lazy_static! {
    static ref STATE: RwLock<HashMap<ChannelId, MessageId>> = {
        // @TUNE Change the number of reserved slots
        // Keeping the space for 500 message IDs is really inexpensive memory-wise (a few
        // kilobytes, maybe a couple dozen kb with the hashmap overhead AT MOST).
        RwLock::new(HashMap::with_capacity(500))
    };

    static ref CONFIG: RwLock<HashMap<GuildId, Server>> = {
        // @TUNE Same here, structs aren't that expensive.
        let file = File::open("data/premade_creator.json");
        if file.is_err() {
            return RwLock::new(HashMap::with_capacity(500));
        }
        let file = file.unwrap();
        RwLock::new(from_reader(file).unwrap_or_else(|e| {
            warn!("couldn't deserialize premade_creator config: {:?}", e);
            HashMap::with_capacity(500)
        }))
    };
}

static mut SCHED_CHANNEL_TX: Option<Sender<()>> = None;

fn rehash(_: &mut Context, _: &Message, _: Args) -> Result<(), CommandError> {
    lazy_static::initialize(&CONFIG);
    unsafe {
        if let Some(ref s) = SCHED_CHANNEL_TX {
            // The other end NEEDS to be connected to work correctly. We can unwrap since it should
            // always be the case.
            s.send(()).unwrap();
        }
    }
    
    msg.channel_id.send_message(|m| m.content("Configuration successfully reloaded."))?;

    Ok(())
}

#[derive(Default)]
pub struct PremadeCreator;

impl Module for PremadeCreator {
    fn register(framework: StandardFramework) -> StandardFramework {
        let (sched_tx, sched_rx) = channel();

        unsafe {
            SCHED_CHANNEL_TX = Some(sched_tx);
        }

        thread::spawn(move || loop {
            let mut sched = JobScheduler::new();

            {
                let config = CONFIG.read().expect("couldn't lock config for reading");

                for (server_id, server) in config.iter() {
                    let sid = *server_id;
                    sched.add(Job::new(
                        server.start.parse().expect("bad start syntax"),
                        move || process_start(sid),
                    ));
                    sched.add(Job::new(
                        server.end.parse().expect("bad end syntax"),
                        move || process_end(sid),
                    ));
                }
            }

            while sched_rx.try_recv().is_err() {
                sched.tick();
                let tick_size = {
                    let settings = SETTINGS.read().expect("couldn't lock settings for reading");
                    Duration::from_secs(
                        settings
                            .get::<u64>("premade-creator.tick")
                            .expect("couldn't find tick length"),
                    )
                };
                thread::sleep(tick_size);
            }
        });

        framework.group("Premade Creator", |g| {
            g.desc("Commands to manipulate the Premade Creator module")
                .required_permissions(Permissions::MANAGE_GUILD)
                .cmd("pmconfig get", creator_command::GetCommand::default())
                .cmd("pmconfig create", creator_command::CreateCommand::default())
                .cmd("pmconfig set", creator_command::SetCommand::default())
                .cmd(
                    "pmconfig add roles",
                    creator_command::AddRolesCommand::default(),
                ).cmd(
                    "pmconfig add game",
                    creator_command::AddGameCommand::default(),
                ).cmd("pmconfig commit", creator_command::CommitCommand::default())
                .command("pmrehash", |c| c.owners_only(true).exec(rehash))
        })
    }
}

/// Convenience function to turn an Option<Vec<RoleId>> into a string mentioning the roles, if not
/// None.
fn role_list_to_mentions(roles: &Option<Vec<RoleId>>) -> String {
    match roles {
        // If there's no role configured, don't @ anyone
        None => "".to_string(),
        // If there's any role configured, mention all of them
        Some(ref ids) => ids
            .iter()
            .map(&RoleId::mention)
            .collect::<Vec<String>>()
            .join(", "),
    }
}

/// Function called at the "start" event, which will go through all the servers in the list to @
/// the proper roles proposing them a few games. Potential players need to react with the proper
/// reactions.
fn process_start(server_id: GuildId) {
    info!(
        "Starting the premade creation process in server {}...",
        server_id
    );

    let config = CONFIG.read().expect("couldn't lock config for reading");
    let config = config.get(&server_id);
    if config.is_none() {
        warn!(
            "process started for server {} but config not found",
            server_id
        );
        return;
    }
    let server = config.unwrap();

    // Yeah I realize I could use the r#""# notation but this is way more readable imo.
    let embed_description = vec![
        "Today, these following games are available!".to_string(),
        "React with the corresponding emoji to participate!\n".to_string(),
    ].join("\n");

    let games = &server.games;
    let mut reactions = Vec::with_capacity(games.len());
    let mut embed_games = Vec::with_capacity(games.len());
    for g in games.iter() {
        reactions.push(g.emoji.clone());
        embed_games.push(format!("{} -> `{}`", g.emoji, g.name));
    }

    let embed_games = embed_games
        .into_iter()
        .try_fold(FoldStrlenState::new(900), &fold_by_strlen)
        .expect("error while creating games message");
    let embed_games = embed_games
        .extract()
        .iter()
        .map(|v| v.join("\n"))
        .collect::<Vec<String>>();

    if embed_games.is_empty() {
        return;
    }

    let embed = CreateEmbed::default();
    let embed = embed
        .color(Colour::from_rgb(120, 17, 176))
        .title("Pick your games!")
        .description(&embed_description)
        .field("Games", &embed_games[0], false)
        .fields(embed_games[1..].iter().map(|g| ("Games (cont)", g, false)));

    let message = CreateMessage::default();
    let message = message.embed(|_| embed).reactions(reactions.into_iter());

    let message = message.content(role_list_to_mentions(&server.role_ids));
    match server.channel_id.send_message(|_| message) {
        // Message successfully sent, keep the ID in memory
        Ok(msg) => {
            let mut state = STATE.write().expect("couldn't lock state for writing");
            state.insert(server.channel_id, msg.id);
        }
        // Message wasn't sent correctly. Forwarding error to user.
        Err(e) => {
            warn!("Couldn't send message to server {}: {:?}", server_id.0, e);
        }
    };
}

/// Function called at the "end" event. Traverses all the list of servers to find out the messages
/// it sent, and writes a message with all players for every particular game.
fn process_end(server_id: GuildId) {
    info!("Ending the premade creation process...");

    let embed = CreateEmbed::default();
    let embed = embed
        .color(Colour::from_rgb(120, 17, 176))
        .title("Today's players")
        .description("The following players want to play:");

    let config = CONFIG.read().expect("couldn't lock config for reading");
    let config = config.get(&server_id);
    if config.is_none() {
        warn!(
            "process started for server {} but config not found",
            server_id
        );
        return;
    }
    let server = config.unwrap();

    let games = &server.games;
    let message_id = {
        let state = STATE.read().expect("couldn't lock state for reading");
        match state.get(&server.channel_id) {
            None => {
                warn!("Initial message not found for server {}!", server_id);
                return;
            }
            Some(mid) => *mid,
        }
    };

    for g in games.iter() {
        let mentions = server
            .channel_id
            .reaction_users(message_id, g.emoji.clone(), None, None)
            .expect("couldn't get reactions");
        let mentions = mentions.iter().filter(|user| !user.bot);
        let mentions = mentions
            .map(&User::mention)
            .try_fold(FoldStrlenState::new(900), &fold_by_strlen)
            .expect("error while making mentions");
        let mut mentions = mentions
            .extract()
            .iter()
            .map(|v| v.join(", "))
            .collect::<Vec<String>>();

        // If nobody answered for this particular game, skip
        if mentions.is_empty() {
            continue;
        }

        let embed = embed.clone();
        let embed = embed
            .field(format!("{} {}", g.emoji, g.name), &mentions[0], false)
            .fields(
                mentions[1..]
                    .iter()
                    .map(|m| (format!("{} {} (cont)", g.emoji, g.name), m, false)),
            );

        let result = g.channel_id.send_message(|m| {
            let message = m.embed(|_| embed);
            message.content(role_list_to_mentions(&g.role_ids))
        });
        if let Err(err) = result {
            warn!(
                "The message couldn't be sent to server {}: {:?}",
                server_id, err
            );
        }
    }

    let mut state = STATE.write().expect("couldn't lock state for writing");
    state.remove(&server.channel_id);
}

use std::fs::OpenOptions;

fn save_config() -> Result<(), String> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .open("data/premade_creator.json");
    let file = file.map_err(|e| e.to_string())?;

    let config = CONFIG.read().expect("couldn't lock CONFIG for writing");
    to_writer_pretty(file, &*config).map_err(|e| format!("{}", e))
}

/// Represents a game info.
/// A game has a name that will represent it everywhere, an emoji used in reactions, a list of
/// roles to be @ed when the game's message is sent, and a channel to send the message to.
#[derive(Clone, Serialize, Deserialize)]
struct GameInfo {
    name: String,
    emoji: ReactionType,
    role_ids: Option<Vec<RoleId>>,
    channel_id: ChannelId,
}

/// Represents a server.
/// A server has a channel id representing the channel to which the messages will sent,
/// start and end strings representing times at which the events will fire (cron syntax),
/// and an optional list of roles to be @ed when the messages are sent.
#[derive(Clone, Serialize, Deserialize, Default)]
struct Server {
    channel_id: ChannelId,
    start: String,
    end: String,
    role_ids: Option<Vec<RoleId>>,
    games: Vec<GameInfo>,
}
