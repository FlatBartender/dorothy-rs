#[macro_use]
extern crate log;
extern crate pretty_env_logger;
extern crate serenity;
extern crate config;
extern crate job_scheduler;
#[macro_use]
extern crate lazy_static;

use std::sync::Arc;
use std::sync::RwLock;

pub mod dorothy;
pub mod utils;
pub mod premade_creator;

lazy_static! {
    static ref SETTINGS: Arc<RwLock<config::Config>> = {
        Arc::new(RwLock::new(config::Config::default()))
    };
}
fn get_settings() -> Arc<RwLock<config::Config>> {
    SETTINGS.clone()
}


fn init_env() {
    pretty_env_logger::init();    
    let settings = get_settings();
    let mut settings = settings.write().expect("couldn't lock the settings for writing");
    settings.merge(config::File::with_name("Settings")).expect("couldn't find the Settings file");
}

fn main() {
    init_env();
    
    let token = {
        let settings = get_settings();
        let settings = settings.read().expect("couldn't get settings");
        settings.get_str("token").expect("couldn't find token")
    };

    let dorothy = dorothy::Dorothy::default();

    let mut client = serenity::client::Client::new(&token, dorothy).expect("couldn't login");

    let framework = serenity::framework::StandardFramework::new();
    let mut framework = framework.configure(|c| c.prefix("d!"));

    let mut modules = Vec::new();
    
    // Register modules here. Simply put the result of their init function in the modules array. 
    
    modules.push(&premade_creator::register);
    
    for register in modules.iter_mut() {
        framework = register(framework);
    }

    client.with_framework(framework);
    
    client.start().expect("couldn't start bot");
}
