use std::{process::Command, collections::HashSet, hash::Hash, sync::MutexGuard};
use crate::signals::Signal;
use glob::glob;
use std::sync::Mutex;
use std::time::Duration;
use std::path::PathBuf;
use log::{debug, info};
use tokio::time;
use std::sync::Arc;

// trait VecSeekOrAppend<T: PartialEq>: Sized {
//     fn get_or_insert(&mut self, item: T) -> &mut T;
// }

// impl<T> VecSeekOrAppend<T> for Vec<T>
// where T: PartialEq +  Clone {

//     fn get_or_insert(&mut self, item: T) -> &mut T {
//         if !self.contains(&item) {
//             self.push(item);
//         }
//         for i in self.iter_mut() {

//         }
//     }
// }
//

pub async fn cache_refresher(cache: Arc<FilesMetadataCacheStore>) {
    cache.refresh_store().await
}

/// Wallpaper directory cache store
///
/// It watches for the changes in the wallpaper directory
pub struct FilesMetadataCacheStore {
    dir: PathBuf,
    ttl: Duration,
    store: Mutex<Vec<PathBuf>>,
    current_id: Mutex<usize>,
}


impl FilesMetadataCacheStore {
    pub fn new(dir: PathBuf, ttl: u64) -> Self {
        if !dir.is_dir() {
            panic!("{} is not a directory", dir.display())
        }

        let store = Self::get_wallpapers(&dir);
        let ttl = Duration::from_secs(ttl);

        Self {
            dir,
            ttl,
            store: Mutex::new(store),
            current_id: Mutex::new(0),
        }
    }

    fn get_matched_files(dir: &PathBuf) -> (impl Iterator<Item = PathBuf>, usize) {
        let glob_patterns = vec![
            format!("{}/**/*.jpg", dir.display()),
            format!("{}/**/*.png", dir.display()),
        ];

        let files = glob(&glob_patterns[0]).unwrap()
            .chain(glob(&glob_patterns[1]).unwrap())
            .filter_map(|f| {
                debug!("File: {:?}", f);
                f.ok()
            });

        let hint = files.size_hint();
        (files, hint.1.unwrap_or(0) - hint.0)
    }

    fn get_wallpapers(dir: &PathBuf) -> Vec<PathBuf> {
        let (files, size) = Self::get_matched_files(dir);

        let mut store: Vec<PathBuf> = Vec::with_capacity(size);
        files
            .for_each(|f| {
                debug!("file to append: {f:?}");
                store.push(f);
            });

        store
    }

    fn ensure_store_capacity_is_enough(store: &mut MutexGuard<Vec<PathBuf>>, needed_size: usize) {
        let store_len = store.len();
        if needed_size > store_len {
            store.reserve(needed_size - store_len)
        }
    }

    pub async fn refresh_store(&self) {
        let mut interval = time::interval(self.ttl);
        loop {

            interval.tick().await;
            let mut store = self.store.lock().unwrap();
            let (files, size) = Self::get_matched_files(&self.dir);
            Self::ensure_store_capacity_is_enough(&mut store, size);

            files
                .for_each(|f| {
                    debug!("file to append: {f:?}");
                    if !store.contains(&f) {
                        store.push(f);
                    }
                })

            }

    }

    pub fn set_background(&self, signal: Signal) {
        let mut id = self.current_id.lock().unwrap();
        let store = self.store.lock().unwrap();
        let store_len = store.len();

        debug!("Old id: {}", *id);
        match signal {
            Signal::Next => {
                // moves head to the end
                // which basically is shifting buffer by 1 to the left
                if store_len > *id + 1 {
                    *id += 1;
                } else {
                    *id = 0;
                }
            },
            Signal::Prev => {
                if 0 > (*id - 1) as i32 {
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

        info!("Successfully changed wallpaper to: {}", store[*id].to_str().unwrap());
    }
}