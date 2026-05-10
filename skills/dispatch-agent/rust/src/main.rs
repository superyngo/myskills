mod cli;
mod config;
mod config_cmd;
mod detect;
mod dispatch;
mod env;
mod fsutil;
mod init;
mod rr_state;
mod templates;
mod types;

use cli::Commands;
use config_cmd::cmd_config;
use detect::cmd_detect;
use dispatch::cmd_dispatch;
use init::cmd_init;

fn main() {
    use clap::Parser;
    let cli = cli::Cli::parse();
    let config_path = cli.global.config.as_deref();

    let result = match cli.command {
        Commands::Detect => cmd_detect(),
        Commands::Init => cmd_init(),
        Commands::Dispatch(args) => cmd_dispatch(&args, config_path),
        Commands::Config(args) => cmd_config(&args, config_path),
    };

    if let Err(e) = result {
        eprintln!("Error: {e:#}");
        std::process::exit(1);
    }
}
