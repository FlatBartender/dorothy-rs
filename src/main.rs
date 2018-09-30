#[macro_use]
extern crate log;
extern crate pretty_env_logger;
extern crate serenity;
extern crate config;
extern crate job_scheduler;
#[macro_use]
extern crate lazy_static;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate cron;

use serenity::framework::StandardFramework;

use std::sync::RwLock;
use std::collections::HashSet;
use std::iter::FromIterator;

pub mod dorothy;
pub mod utils;
pub mod premade_creator;
pub mod misc;

use premade_creator::PremadeCreator;
use misc::Misc;
use dorothy::Module;

lazy_static! {
    static ref SETTINGS: RwLock<config::Config> = {
        RwLock::new(config::Config::default())
    };
}

fn init_env() {
    pretty_env_logger::init();    
    let mut settings = SETTINGS.write().expect("couldn't lock the settings for writing");
    settings.merge(config::File::with_name("Settings")).expect("couldn't find the Settings file");
}

fn main() {
    init_env();
    
    let (token, owners, prefix) = {
        let settings = SETTINGS.read().expect("couldn't get settings");
        let token = settings.get_str("token").expect("couldn't find token");
        let owners = settings.get_array("owners").expect("couldn't find owners");
        let owners = owners.into_iter().map(|o| o.try_into().expect("couldn't get owner ID"));
        let prefix = settings.get_str("prefix").expect("couldn(t finf prefix");

        (token, HashSet::from_iter(owners), prefix)
    };

    let dorothy = dorothy::Dorothy::default();

    let mut client = serenity::client::Client::new(&token, dorothy).expect("couldn't login");

    let framework = serenity::framework::StandardFramework::default();
    let mut framework = framework.configure(|c| c.prefix(&prefix)
                                            .owners(owners))
        .on_dispatch_error(|_, _, e| warn!("Dispatch error: {:?}", e))
        .help(serenity::framework::standard::help_commands::with_embeds);

    let mut modules: Vec<Box<Fn(StandardFramework) -> StandardFramework>> = Vec::new();
    
    // Register modules here. Simply put the result of their init function in the modules array. 
    
    modules.push(Box::new(&PremadeCreator::register));
    modules.push(Box::new(&Misc::register));
    
    for register in modules.iter_mut() {
        framework = register(framework);
    }

    client.with_framework(framework);

    client.start().expect("couldn't start bot");
}
