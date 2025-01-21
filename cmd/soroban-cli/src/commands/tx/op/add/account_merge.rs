use clap::{command, Parser};

use std::fmt::Debug;

use super::new;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub args: super::args::Args,
    #[command(flatten)]
    pub op: new::account_merge::Args,
}
