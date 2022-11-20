use std::path::{PathBuf, Path};
use std::time::Duration;
use clap::{Parser, Subcommand, Args, ValueEnum};
use lib::{daemon, client};
use lib::REFRESH_INTERVAl_IN_SECS;
use lib::signals::Signal;

/// Daemon which changes wallpapers from provided directory with a time interval
#[derive(Parser, Debug)]
struct StartArgs {
    /// Directory of the wallpapers
    #[arg(short = 'd', long, env = "WPCYCLER_DIR")]
    wallpapers_directory: PathBuf,

    /// Intervals between changing wallpapers
    #[arg(short, long, value_name = "WPCYCLER_REFRESH_INTERVAL", default_value_t = REFRESH_INTERVAl_IN_SECS)]
    refresh_interval: u64,
}

/// Program to set wallpapers from directory based on previous applications
/// they shouldn't repeat unless all of them have already been presented in specific round
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Start(StartArgs),
    Do(DoArgs),
}

/// Client might interact with the daemon through that subcommand
///
/// It allows to send signal to the daemon
#[derive(Parser, Debug)]
struct DoArgs {
    #[arg(value_enum)]
    signal: Signal
}


#[tokio::main]
async fn main() {
    // Setup logger
    let env = env_logger::Env::default();
    env_logger::init_from_env(env);

    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Start(args) => {

            daemon::start(args.wallpapers_directory, Duration::from_secs(args.refresh_interval)).await;
        },
        Cmd::Do(args) => {
            client::invoke(args.signal);
        },
    };

}
