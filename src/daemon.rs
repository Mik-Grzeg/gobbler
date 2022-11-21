use std::fs::{read_dir, DirEntry};
use std::task::Waker;
use super::shutdown::Shutdown;
use std::{time::Duration, io::Read};
use std::path::{PathBuf, Path};
use futures::Future;
use signal_hook::{consts::{SIGINT, SIGTERM, SIGQUIT}, iterator::Signals};

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



/// PipeStream is a structure which hides the logic of reading file in a loop
///
/// It implements Stream, so it can be processed asynchronously
struct PipeStream {
    /// Unix named pipe file
    f: File,

    /// Configuration of bincode that is used to encode/decode data sent by client
    config: Configuration,

    /// Wakes up the stream to try to poll the future in case of of [Poll::Pending] state
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

/// Handles signals invoked by client
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

/// Starts asynchronous tasks:
///
/// * changing wallpapers
/// * watching for new wallpapers in the provided directory
/// * listens to client events which can be triggered by the user
async fn start(walpapers_dir: PathBuf, refresh_interval: Duration, cache_ttl: Duration, pipe_path: &Path) -> std::io::Result<()> {
    let (tx, _) = broadcast::channel(1);
    let cache = Arc::new(FilesMetadataCacheStore::new(walpapers_dir, cache_ttl));
    let read = unix_named_pipe::open_read(pipe_path)?;
    let config = bincode::config::standard();

    let pipe_stream = PipeStream::new(read, config);

    let client_task = task::spawn({
        let cache = cache.clone();
        client_signal_handler(pipe_stream, cache.clone(), Shutdown::new(tx.subscribe()))
    });

    let file_store_refresh_task = task::spawn({
        let cache = cache.clone();
        let rx = tx.subscribe();
        async move { cache.refresh_store(Shutdown::new(rx)).await }
    });

    let background_changer_task = task::spawn({
        let cache = cache.clone();
        let rx = tx.subscribe();
        async move { cache.start_background_changer(refresh_interval, Shutdown::new(rx)).await }
    });

    let mut shutdown_signals = Signals::new(&[SIGINT, SIGTERM])?;

    let shutdown_signal_watcher = task::spawn(async move {
       for signal in shutdown_signals.forever() {
            match signal {
                SIGTERM | SIGINT | SIGQUIT => {
                    warn!("Shutting down the daemon with signal: {signal}");
                    return
                },
                _ => unreachable!(),
            }
        }
    });


    tokio::select! {
        _ = shutdown_signal_watcher => { },
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

    tx.send(());
    Ok(())
}

/// Initializing unix named pipe and daemon. It is also responsible for removing named piped after
/// terminating the program
pub async fn init(wallpapers_dir: PathBuf, refresh_interval: Duration, cache_ttl: Duration) {
    let pipe_path = UNIX_PIPE_FILE_NAME.as_path();
    unix_named_pipe::create(pipe_path, Some(0o740)).unwrap();
    info!("Pipe has been created at: {}", pipe_path.display());

    if let Err(why) = start(wallpapers_dir, refresh_interval, cache_ttl, pipe_path).await {
        error!("IO error: {why}");
    }

    fs::remove_file(pipe_path).expect("could not remove pipe file");
    info!("Removed pipe {} successfully", pipe_path.display());
}
