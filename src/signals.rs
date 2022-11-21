use bincode::{Decode, Encode};
use clap::ValueEnum;

/// Signal to propagate to the daemon
///
/// If the daemon is not running, it will communicate proper message, that it has to be running
#[derive(ValueEnum, Encode, Decode, Clone, Debug)]
pub enum Signal {
    /// Allows to notify wallpaper switching daemon to change to the next wallpaper
    Next = 1,

    /// Allows to notify wallpaper switching daemon to change to the previous wallpaper
    Prev = 2,
}
