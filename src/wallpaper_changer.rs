use crate::{shutdown::Shutdown, signals::Signal};
use glob::glob;
use log::{debug, info, warn};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use std::{process::Command, sync::MutexGuard};
use tokio::time;

/// Wallpaper directory cache store
///
/// It watches for the changes in the wallpaper directory
/// and changes the wallpapers
pub struct FilesMetadataCacheStore {
    /// Wallpaper directory
    dir: PathBuf,

    /// How often should cache be refreshed
    ttl: Duration,

    /// Cache store which keeps names of the files that matches *.(png|jpg) format
    store: Mutex<Vec<PathBuf>>,

    /// Id of the current wallpaper
    current_id: Mutex<usize>,
}

impl FilesMetadataCacheStore {
    /// Constructor
    pub fn new(dir: PathBuf, ttl: Duration) -> Self {
        if !dir.is_dir() {
            panic!("{} is not a directory", dir.display())
        }

        let store = Self::get_wallpapers(&dir);

        Self {
            dir,
            ttl,
            store: Mutex::new(store),
            current_id: Mutex::new(0),
        }
    }

    /// Searches for the jpg and png files in a directory
    fn get_matched_files(dir: &PathBuf) -> (impl Iterator<Item = PathBuf>, usize) {
        let glob_patterns = vec![
            format!("{}/**/*.jpg", dir.display()),
            format!("{}/**/*.png", dir.display()),
        ];

        let files = glob(&glob_patterns[0])
            .unwrap()
            .chain(glob(&glob_patterns[1]).unwrap())
            .filter_map(|f| {
                debug!("File: {:?}", f);
                f.ok()
            });

        let hint = files.size_hint();
        (files, hint.1.unwrap_or(0) - hint.0)
    }

    /// Gets names of the wallpaper files in a directory
    fn get_wallpapers(dir: &PathBuf) -> Vec<PathBuf> {
        let (files, size) = Self::get_matched_files(dir);

        let mut store: Vec<PathBuf> = Vec::with_capacity(size);
        files.for_each(|f| {
            debug!("file to append: {f:?}");
            store.push(f);
        });

        store
    }

    /// Ensures that the cache has enough capacity for the refreshed list of wallpapers
    fn ensure_store_capacity_is_enough(store: &mut MutexGuard<Vec<PathBuf>>, needed_size: usize) {
        let store_len = store.len();
        if needed_size > store_len {
            store.reserve(needed_size - store_len)
        }
    }

    /// Reads filenames in the directory and adds new files to the cache store
    /// in case of a new ones
    pub async fn refresh_store(&self, mut shutdown: Shutdown) {
        let mut interval = time::interval(self.ttl);
        while !shutdown.is_shutdown() {
            debug!(target: "refresh_store_task", "{}", shutdown.is_shutdown());

            tokio::select! {
                _ = interval.tick() => {},
                _ = shutdown.recv() => {
                    warn!(target: "refresh_store_task", "received shutdown");
                    return
                }
            }

            let mut store = self.store.lock().unwrap();
            let (files, size) = Self::get_matched_files(&self.dir);
            Self::ensure_store_capacity_is_enough(&mut store, size);

            files.for_each(|f| {
                debug!("file to append: {f:?}");
                if !store.contains(&f) {
                    store.push(f);
                }
            })
        }
    }

    /// Responsible for changing wallpapers
    ///
    /// It sleeps for the duration of provided interval and then sets new background
    pub async fn start_background_changer(
        &self,
        refresh_interval: Duration,
        mut shutdown: Shutdown,
    ) {
        let mut interval = time::interval(refresh_interval);

        while !shutdown.is_shutdown() {
            debug!(target: "background_changer_task", "{}", shutdown.is_shutdown());
            tokio::select! {
                _ = interval.tick() => {
                    self.set_background(Signal::Next);
                },
                _ = shutdown.recv() => {
                    warn!(target: "start_background_changer_task", "received shutdown");
                    return
                }
            }
        }
    }

    /// Sets new background based on input signal.
    /// It can be next or previous one.
    ///
    ///
    /// Keeps track of the current background and based on that and input signal
    /// decides which wallpaper is being set.
    pub fn set_background(&self, signal: Signal) {
        let mut id = self.current_id.lock().unwrap();
        let store = self.store.lock().unwrap();
        let store_len = store.len();

        debug!("Old id: {}", *id);
        match signal {
            Signal::Next => {
                // moves head to the end
                // which basically is shifting buffer by 1 to the left
                if store_len - 1 > *id {
                    *id += 1;
                } else {
                    *id = 0;
                }
            }
            Signal::Prev => {
                if *id > 0 {
                    *id -= 1;
                } else {
                    *id = store_len - 1;
                }
            }
        };
        debug!("New id: {}", *id);

        Command::new("feh")
            .args(["--bg-scale", store[*id].to_str().unwrap()])
            .status()
            .expect("unable to set wallpaper with feh");

        info!(
            "Successfully changed wallpaper to: {}",
            store[*id].to_str().unwrap()
        );
    }
}
