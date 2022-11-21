use std::fs::{read_dir, DirEntry};
use std::task::Waker;
use super::shutdown::Shutdown;
use std::{time::Duration, io::Read};
use std::path::{PathBuf, Path};
use futures::Future;

use super::consts::UNIX_PIPE_FILE_NAME;
use super::consts::CACHE_STORE_TTL;
use bincode::config::{Config, Configuration};
use bincode::error::DecodeError;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::broadcast;
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
use std::cell::RefCell;



struct PipeStream {
    f: File,
    config: Configuration,
    waker: Option<Waker>,
}

impl PipeStream {
    fn new(f: File, config: Configuration) -> Self {
        Self {
            f,
            config,
            waker: None
        }
    }
}

impl Future for PipeStream {
    type Output = Signal;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let slf = self.get_mut();
        if let Ok(s) =  bincode::decode_from_std_read(&mut slf.f, slf.config) {
            return Poll::Ready(s)
        } else {
            slf.waker = Some(cx.waker().clone());
            return Poll::Pending
        }
    }
}

impl Stream for PipeStream {
    type Item = Signal;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>
        ) -> Poll<Option<Self::Item>> {

        let mut slf = self.get_mut();

        let polled = Pin::new(&mut slf).poll(cx);
        match polled {
            Poll::Ready(s) => return Poll::Ready(Some(s)),
            Poll::Pending => slf.waker.as_ref().unwrap().wake_by_ref()
        }

        sleep(Duration::from_millis(50));
        Poll::Pending
    }
}

async fn client_signal_handler(mut pipe_stream: PipeStream, cache: Arc<FilesMetadataCacheStore>, mut shutdown: Shutdown) {
    loop {
        debug!(target: "client_task", "{}", shutdown.is_shutdown());
        let signal = tokio::select! {
            output = pipe_stream.next() => {
                match output {
                    Some(s) => s,
                    None => return
                }
            },
            _ = shutdown.recv() => {
                warn!(target: "client_task", "received shutdown");
                return
            }
        };
        info!("Manually invoked wallpaper change to: {signal:?}");
        cache.set_background(signal);
    }
}

async fn start(walpapers_dir: PathBuf, refresh_interval: Duration, cache_ttl: Duration, pipe_path: &Path,
               shutdown: Sender<()>) -> std::io::Result<()> {

    let cache = Arc::new(FilesMetadataCacheStore::new(walpapers_dir, cache_ttl));
    let read = unix_named_pipe::open_read(pipe_path)?;
    let config = bincode::config::standard();

    let pipe_stream = PipeStream::new(read, config);

    let client_task = task::spawn({
        let cache = cache.clone();
        client_signal_handler(pipe_stream, cache.clone(), Shutdown::new(shutdown.subscribe(), "client_task".to_owned()))
        // let mut kill = kill.subscribe();
        // async move {
        //     tokio::select! {
        //         r = kill.recv() => {
        //             warn!(target: "client_singlas_receive_task", "Shutting down: {r:?}");
        //         },
        //         output = client_signal_handler(pipe_stream, cache.clone()) => output
        //     }
        // }
    });
    let file_store_refresh_task = task::spawn({
        let cache = cache.clone();
        let rx = shutdown.subscribe();
        async move { cache.refresh_store(Shutdown::new(rx, "refresh_store_task".to_owned())).await }
        // let mut kill = kill.subscribe();
        // async move {
        //     tokio::select! {
        //         r = kill.recv() => {
        //             warn!(target: "file_store_refresh_task", "Shutting down: {:?}", r);
        //             drop(r)
        //         },
        //         output = cache.refresh_store() => output
        //     }
        // }
    });
    let background_changer_task = task::spawn({
        let cache = cache.clone();
        let rx = shutdown.subscribe();
        async move { cache.start_background_changer(refresh_interval, Shutdown::new(rx, "background_changer_task".to_owned())).await }
        // let mut kill = kill.subscribe();
        // async move {
        //     tokio::select! {
        //         r = kill.recv() => {
        //             warn!(target: "background_changer_task", "Shutting down: {r:?}");
        //         },
        //         output = cache.start_background_changer(refresh_interval) => output
        //     }
        // }
    });


    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            warn!("Shutting down the daemon");
        },
        _ = background_changer_task => {
            warn!(target: "background_changer_task", "Shutting down");
        },
        _ = client_task => {
            warn!(target: "client_task", "Shutting down");
        },
        _ = file_store_refresh_task => {
            warn!(target: "file_store_refresh_task", "Shutting down");
        },
    }

    shutdown.send(()).unwrap();
    Ok(())
}

pub async fn init(wallpapers_dir: PathBuf, refresh_interval: Duration, cache_ttl: Duration) {
    let pipe_path = UNIX_PIPE_FILE_NAME.as_path();
    unix_named_pipe::create(pipe_path, Some(0o740)).unwrap();
    info!("Pipe has been created at: {}", pipe_path.display());

    defer! {
        fs::remove_file(pipe_path).expect("could not remove pipe file");
        info!("Removed {} successfully", pipe_path.display());
    }

    let (tx, _) = broadcast::channel(1);

    start(wallpapers_dir, refresh_interval, cache_ttl, pipe_path, tx.clone()).await.unwrap()
}
