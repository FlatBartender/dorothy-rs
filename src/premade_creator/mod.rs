/// This module is a bit different from others.
/// Since it operates not based on messages but on time events, it doesn't register a struct in
/// the handlers array but instead the state is kept in a static variable.
///
/// It needs to find the `premade-creator.start` and the `premade-creator.end` strings in the
/// config, syntax is cron (see job_scheduler crate documentation).
/// At the start event it will @everyone with a list of games associated with emojis 
/// under `premade-creator.games` (as key-value pairs, game name -> [emoji*, team size]).
/// At the end event, it will look for reactions on the message posted for the start, form teams
/// with people who reacted to the corresponding emoji, send multiple messages to the teams, and @
/// the remaining people who haven't been chosen.
/// The job scheduler will check every `premade-creator.tick` seconds (int) for the events.
/// `premade-creator.servers` will be a table associating a server ID in a string (can't use u64s
/// as keys) with channel id as value. For each of these servers, the bot will send the message on
/// the specified channel.
///
/// * an emoji is either a string (for unicode emojis) or an array [name, id].
use job_scheduler::{JobScheduler, Job};

use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::framework::StandardFramework;
use serenity::utils::Colour;
use serenity::builder::*;

use std::thread;
use std::time::Duration;
use std::sync::{Arc, RwLock};

use get_settings;
use utils::*;

static mut STATE: Option<Arc<RwLock<PremadeCreator>>> = None;

#[doc(hidden)]
fn get_state() -> Arc<RwLock<PremadeCreator>> {
    unsafe {
        STATE.clone().expect("couldn't get state")
    }
}

pub fn register(framework: StandardFramework) -> StandardFramework {
    let (start_time, end_time, games, servers) = {
        let settings = get_settings();
        let settings = settings.read().expect("couldn't read settings");

        let start = settings.get_str("premade-creator.start").expect("couldn't get cron line for start event");
        let end   = settings.get_str("premade-creator.end").expect("couldn't get cron line for end event");

        let games = settings.get_array("premade-creator.games").expect("couldn't get games");

        let games = games.into_iter().map(|v| {
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
        }).collect();
 
        let servers = settings.get_table("premade-creator.servers").expect("couldn't get servers");
        let servers = servers.into_iter().map(|(server, channel)| {
            let server_id = GuildId(server.parse().expect("couldn't parse server string"));
            let channel_id = ChannelId(channel.try_into().expect("couldn't deserialize channel"));

            Server {
                server_id,
                channel_id,
                message: None,
            }
        }).collect();

        (start, end, games, servers)
    };

    let state = PremadeCreator {
        games,
        servers,
    };
    
    unsafe {
        STATE = Some(Arc::new(RwLock::new(state)));
    }

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

fn process_start() {
    info!("Starting the premade creation process...");

    let state = get_state();
    let mut state = state.write().expect("couldn't lock state");
    
    // Yeah I realize I could use the r#""# notation but this is way more readable imo.
    let embed_description = vec![
        "Today, these following games are available!".to_string(),
        "React with the corresponding emoji to participate!\n".to_string(),
    ].join("\n");


    let mut reactions   = Vec::with_capacity(state.games.len());
    let mut embed_games = Vec::with_capacity(state.games.len());
    for g in state.games.iter() {
        reactions.push(g.emoji.clone());
        embed_games.push(format!("{} -> `{}`", g.emoji, g.name));
    }

    let embed_games = embed_games.into_iter().try_fold(FoldStrlenState::new(1024), &fold_by_strlen).expect("error while creating games message");
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
        .content("@everyone")
        .reactions(reactions.into_iter());

    for s in state.servers.iter_mut() {
        // @TODO fix the @@everyone problem
        let result = s.channel_id.send_message(|_| message.clone());
        match result {
            // Message successfully sent, keeping the ID in memory
            Ok(msg) => {
                s.message = Some(msg.id);
            },
            // Message wasn't sent correctly. Forwarding error to user.
            Err(e) => {
                warn!("Couldn't send message to server {}: {:?}", s.server_id.0, e);
            }
        };
    }
}

fn process_end() {
    info!("Ending the premade creation process...");

    let state = get_state();
    let mut state = state.write().expect("couldn't lock state");

    let embed = CreateEmbed::default();
    let embed = embed.color(Colour::from_rgb(120, 17, 176))
                 .title("Today's players")
                 .description("The following players want to play:");

    for s in state.servers.iter() {
        if let None = s.message {
            warn!("Initial message not found for server {}!", s.server_id);
            continue;
        } 
        let message_id = s.message.unwrap();
        let mut embed = embed.clone();
        for g in state.games.iter() {
            let mentions = s.channel_id.reaction_users(message_id, g.emoji.clone(), None, None).expect("couldn't get emojis");
            let mentions = mentions.iter().filter(|user| !user.bot);
            let mentions = mentions.map(&User::mention).try_fold(FoldStrlenState::new(1024), &fold_by_strlen).expect("error while making mentions");
            let mut mentions = mentions.extract().iter().map(|v| v.join(", ")).collect::<Vec<String>>();
            
            // If nobody answered for this particular game, skip
            if mentions.len() == 0 {
                continue;
            }

            embed = embed.field(format!("{} {}", g.emoji, g.name), mentions[0].clone(), false)
                .fields(mentions[1..].iter().map(|m| (format!("{} {} (cont)", g.emoji, g.name), m, false)));

        }
        
        s.channel_id.send_message(|m| {
            m.embed(|_| embed)
        }).expect("the message couldn't be sent");

    }

    for s in state.servers.iter_mut() {
        s.message = None;
    }
}

#[derive(Clone)]
struct GameInfo {
    name: String,
    emoji: ReactionType,
    team_size: u32,
}

#[derive(Clone)]
struct Server {
    server_id: GuildId,
    channel_id: ChannelId,
    message: Option<MessageId>,
}

/// The PremadeCreator struct keeps track of the messages used for getting user game choices and
/// various state/configuration variables.
struct PremadeCreator {
    games: Vec<GameInfo>,
    servers: Vec<Server>,
}
