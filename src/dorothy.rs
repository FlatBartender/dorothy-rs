use serenity::framework::StandardFramework;
use serenity::model::prelude::*;
use serenity::prelude::*;

pub trait Module {
    fn register(framework: StandardFramework) -> StandardFramework;
}

#[derive(Default)]
pub struct Dorothy;

impl EventHandler for Dorothy {
    fn ready(&self, _ctx: Context, _data: Ready) {
        info!("NOBODY EXPECTS THE DOROTHINQUISITION!!!");
    }
}
