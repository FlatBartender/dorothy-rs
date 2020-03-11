use serenity::framework::standard::*;
use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::builder::*;

use serenity::framework::standard::macros::{command, group};

use clap::{App, Arg, AppSettings};

use std::str::FromStr;

#[group]
#[owners_only]
pub struct Admin;

