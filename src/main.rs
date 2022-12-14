use clap::{Parser, Subcommand};
use gobbler::signals::Signal;
use daemonize::Daemonize;
use std::fs::File;
use gobbler::REFRESH_INTERVAL_IN_SECS;
use gobbler::CACHE_STORE_TTL;
use gobbler::{client, daemon};
use log::LevelFilter;
use log::info;
use tokio::runtime::Runtime;
use std::path::PathBuf;
use std::time::Duration;

/// Daemon which changes wallpapers from provided directory with a time interval
///
/// * changing wallpapers
/// * watching for new wallpapers in provided directory by the user
/// * listens to client events which can be triggered by the user
#[derive(Parser, Debug)]
struct StartArgs {
    /// Directory of the wallpapers
    #[arg(short = 'd', long, env = "GOBBLER_DIR")]
    wallpapers_directory: PathBuf,

    /// Intervals between changing wallpapers in seconds
    #[arg(long, value_name = "GOBBLER_REFRESH_INTERVAL", default_value_t = REFRESH_INTERVAL_IN_SECS)]
    refresh_interval: u64,

    /// Intervals between fetching list of files in wallpapers_directory in seconds
    #[arg(long = "wallpapers-directory-refresh-interval", value_name = "GOBBLER_REFRESH_WALLPAPERS_DIR_INTERVAL", default_value_t = CACHE_STORE_TTL)]
    cache_ttl: u64,
}
/// Simple wallpaper changer for X11 based standalone window managers.
/// It requires a tool called 'feh' (https://feh.finalrewind.org/) to set wallpapers.
///
/// Allows to spawn a daemon which is responsible for
///
/// * changing wallpapers
/// * watching for new wallpapers in provided directory by the user
/// * listens to client events which can be triggered by the user
#[derive(Parser)]
#[command(author, version, about)]
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
    signal: Signal,
}

fn main() -> std::io::Result<()> {

    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Start(args) => {
            let stdout = File::create("/tmp/gobbler.out").unwrap();
            let stderr = File::create("/tmp/gobbler.err").unwrap();

            let daemonize = Daemonize::new()
                .pid_file("/tmp/gobbler.pid")
                .chown_pid_file(true)
                .working_directory("/tmp")
                .stdout(stdout)
                .stderr(stderr);


            match daemonize.start() {
                Ok(_) => {
                    let rt = Runtime::new().unwrap();
                    rt.block_on(async move {
                        println!("Success, daemonized");
                        //
                        // Setup logger
                        let env = env_logger::Env::default().default_filter_or("info");
                        env_logger::init_from_env(env);

                        let refresh_interval = Duration::from_secs(args.refresh_interval);
                        let cache_ttl = Duration::from_secs(args.cache_ttl);
                        daemon::init(args.wallpapers_directory, refresh_interval, cache_ttl).await;
                        info!("Daemon shut down");
                    });
                },
                Err(e) => eprintln!("Error, {}", e),
            }
            Ok(())
        }
        Cmd::Do(args) => {
            client::invoke(args.signal);
            Ok(())
        }
    }
}
