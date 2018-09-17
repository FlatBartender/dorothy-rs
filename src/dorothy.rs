use serenity::prelude::*;
use serenity::model::prelude::*;

#[derive(Default)]
pub struct Dorothy;

impl EventHandler for Dorothy {
    fn ready(&self, _ctx: Context, _data: Ready) {
        info!("NOBODY EXPECTS THE DOROTHINQUISITION!!!");
    }
}
