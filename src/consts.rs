use lazy_static::lazy_static;
use std::path::PathBuf;

/// Intervals between changing wallpapers
pub const REFRESH_INTERVAl_IN_SECS: u64 = 60; // 3 minutes

pub const CACHE_STORE_TTL: u64 = 600;

/// Name of the package
const PKG_NAME: &str = env!("CARGO_PKG_NAME");


lazy_static! {
    /// Unix pipe file name
    pub static ref UNIX_PIPE_FILE_NAME: PathBuf = PathBuf::from(&format!("/tmp/{}", PKG_NAME));
}
