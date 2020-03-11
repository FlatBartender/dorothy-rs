#[macro_use]
extern crate log;
extern crate config;
extern crate job_scheduler;
extern crate pretty_env_logger;
extern crate serenity;
#[macro_use]
extern crate lazy_static;
extern crate cron;
extern crate serde;
extern crate serde_derive;
extern crate serde_json;
extern crate clap;

use std::{collections::HashSet, iter::FromIterator, sync::RwLock};

use serenity::framework::standard::StandardFramework;

pub mod dorothy;

pub mod misc;
pub mod utils;
//pub mod premade_creator;
pub mod admin;

use dorothy::{command_error_logger, dispatch_error_handler, print_command_used, normal_message, EMBED_HELP};

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

    client.with_framework(
        StandardFramework::new()
            .configure(|c| c.prefix(&prefix).owners(owners))
            .before(print_command_used)
            .after(command_error_logger)
            .on_dispatch_error(dispatch_error_handler)
            .normal_message(normal_message)
            .help(&EMBED_HELP)
            .group(&misc::MISC_GROUP)
            .group(&admin::ADMIN_GROUP)
            //.group(&premade_creator::PREMADECREATOR_GROUP)
    );

    client.start().expect("couldn't start bot");
}
