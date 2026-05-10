use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "dispatch-agent")]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Args)]
pub struct GlobalArgs {
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    Detect,
    Init,
    Dispatch(DispatchArgs),
    Config(ConfigArgs),
}

#[derive(Args)]
pub struct DispatchArgs {
    #[arg(short = 'p', long, group = "prompt_src")]
    pub prompt: Option<String>,
    #[arg(short = 'f', long, group = "prompt_src", value_name = "FILE")]
    pub file: Option<PathBuf>,
    #[arg(long, default_value = "-1")]
    pub timeout: i64,
    #[arg(long, group = "target")]
    pub tier: Option<String>,
    #[arg(long, group = "target")]
    pub agent: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub list: bool,
    #[arg(long)]
    pub show_config: bool,
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Args)]
pub struct ConfigArgs {
    pub action: Option<String>, // edit | show | path
}
