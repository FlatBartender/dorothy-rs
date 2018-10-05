#[macro_use]
extern crate log;
extern crate config;
extern crate job_scheduler;
extern crate pretty_env_logger;
extern crate serenity;
#[macro_use]
extern crate lazy_static;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate cron;
extern crate serde_json;

use serenity::framework::standard::CommandError;
use serenity::framework::standard::DispatchError;
use serenity::framework::StandardFramework;
use serenity::model::prelude::*;
use serenity::prelude::*;

use std::collections::HashSet;
use std::iter::FromIterator;
use std::sync::RwLock;

pub mod dorothy;
pub mod misc;
pub mod premade_creator;
pub mod utils;

use dorothy::Module;
use misc::Misc;
use premade_creator::PremadeCreator;

lazy_static! {
    static ref SETTINGS: RwLock<config::Config> = { RwLock::new(config::Config::default()) };
}

fn init_env() {
    pretty_env_logger::init();
    let mut settings = SETTINGS
        .write()
        .expect("couldn't lock the settings for writing");
    settings
        .merge(config::File::with_name("Settings"))
        .expect("couldn't find the Settings file");
}

fn main() {
    init_env();

    let (token, owners, prefix) = {
        let settings = SETTINGS.read().expect("couldn't get settings");
        let token = settings.get_str("token").expect("couldn't find token");
        let owners = settings.get_array("owners").expect("couldn't find owners");
        let owners = owners
            .into_iter()
            .map(|o| o.try_into().expect("couldn't get owner ID"));
        let prefix = settings.get_str("prefix").expect("couldn(t finf prefix");

        (token, HashSet::from_iter(owners), prefix)
    };

    let dorothy = dorothy::Dorothy::default();

    let mut client = serenity::client::Client::new(&token, dorothy).expect("couldn't login");

    let framework = serenity::framework::StandardFramework::default();
    let mut framework = framework
        .configure(|c| c.prefix(&prefix).owners(owners))
        .before(print_command_used)
        .after(command_error_logger)
        .on_dispatch_error(dispatch_error_handler)
        .help(serenity::framework::standard::help_commands::with_embeds);

    let mut modules: Vec<Box<Fn(StandardFramework) -> StandardFramework>> = Vec::new();

    // Register modules here. Simply put the result of their init function in the modules array.

    modules.push(Box::new(&PremadeCreator::register));
    modules.push(Box::new(&Misc::register));

    for register in &mut modules {
        framework = register(framework);
    }

    client.with_framework(framework);

    client.start().expect("couldn't start bot");
}

fn print_command_used(_ctx: &mut Context, msg: &Message, cmd_name: &str) -> bool {
    info!(
        "{} ({}) used {} in server {:?}, channel {:?}",
        msg.author.name,
        msg.author.id,
        cmd_name,
        msg.guild_id,
        msg.channel_id.name()
    );

    true
}

fn command_error_logger(
    _ctx: &mut Context,
    msg: &Message,
    cmd_name: &str,
    result: Result<(), CommandError>,
) {
    if let Err(e) = result {
        warn!("Command error for command {}: {:?}", cmd_name, e);
        if let Err(e) = msg
            .channel_id
            .send_message(|m| m.content(format!("An error has occurred: `{:#?}`", e)))
        {
            warn!(
                "An error has occurred while sending the error message (lol): {:?}",
                e
            );
        }
    }
}

// Need to prevent clippy on this function because it will warn for variable not being consumed,
// even if we don't control the signature of this function
#[cfg_attr(feature = "cargo-clippy", allow(clippy_style))]
fn dispatch_error_handler(_ctx: Context, msg: Message, err: DispatchError) {
    warn!("An error occurred: {:?}", err);
    if let Err(e) = msg
        .channel_id
        .send_message(|m| m.content(format!("An error has occurred: `{:#?}`", err)))
    {
        warn!(
            "An error has occurred while sending the error message (lol): {:?}",
            e
        );
    }
}
