use std::fs::{read_dir, DirEntry};
use std::thread::sleep_ms;
use std::{time::Duration, io::Read};
use std::path::{PathBuf, Path};
use crate::wallpaper_changer::cache_refresher;

use super::consts::UNIX_PIPE_FILE_NAME;
use super::consts::CACHE_STORE_TTL;
use bincode::Decode;
use bincode::config::{Config, Configuration};
use bincode::error::DecodeError;
use log::{debug, info, warn, error};
use scopeguard::defer;
use serde::Serialize;
use super::signals::Signal;
use std::{fs::{self, File}};
use std::io::{Write, Seek};
use std::thread::sleep;
use futures::stream::{Stream, StreamExt};
use futures::task::{Context, Poll};
use std::process::Command;
use std::pin::Pin;
use queues::{CircularBuffer, IsQueue};
use glob::glob;
use std::sync::{Arc, Mutex};
use tokio::{task, time};
use super::wallpaper_changer::FilesMetadataCacheStore;



struct PipeStream {
    f: File,
    config: Configuration,
}

impl PipeStream {
    fn new(f: File, config: Configuration) -> Self {
        Self {
            f,
            config
        }
    }
}

impl Stream for PipeStream {
    type Item = Signal;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>
        ) -> Poll<Option<Self::Item>> {

        let slf = self.get_mut();

        debug!("Checking jazda");
        loop {
            if let Ok(s) =  bincode::decode_from_std_read(&mut slf.f, slf.config) {
                return Poll::Ready(Some(s))
            };
            sleep(Duration::from_millis(50));
        }
    }
}

async fn client_signal_handler(mut pipe_stream: PipeStream, cache: Arc<FilesMetadataCacheStore>) {
    while let Some(signal) = pipe_stream.next().await {
        info!("Manually invoked wallpaper change to: {signal:?}");
        cache.set_background(signal);
    }
}

pub async fn start(walpapers_dir: PathBuf, refresh_interval: Duration) {
    let pipe_path = UNIX_PIPE_FILE_NAME.as_path();
    unix_named_pipe::create(pipe_path, Some(0o740)).unwrap();
    info!("Pipe has been created at: {}", pipe_path.display());

    defer! {
        fs::remove_file(pipe_path).expect("could not remove pipe file");
        info!("Removed {} successfully", pipe_path.display());
    }

    let cache = Arc::new(FilesMetadataCacheStore::new(walpapers_dir, CACHE_STORE_TTL));
    let read = unix_named_pipe::open_read(pipe_path).unwrap();
    let config = bincode::config::standard();

    let pipe_stream = PipeStream::new(read, config);

    let client_task = task::spawn(client_signal_handler(pipe_stream, cache.clone()));
    let file_store_refresh_task = task::spawn(cache_refresher(cache.clone()));

    let mut interval = time::interval(refresh_interval);

    loop {
        interval.tick().await;
        cache.set_background(Signal::Next);
    }

    tokio::join!(client_task, file_store_refresh_task);
}
