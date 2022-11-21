use tokio::sync::broadcast::Receiver;

/// Shutdown helper which allows to propagate termination signal
/// amongst asynchronous tasks
pub struct Shutdown {
    /// False until receiver receives an event
    kill: bool,

    /// Broadcast receiver, which awaits for the first event
    receiver: Receiver<()>,
}

impl Shutdown {
    /// Constructor
    pub fn new(receiver: Receiver<()>) -> Self {
        Self {
            receiver,
            kill: false,
        }
    }

    /// Return state of the receiver
    pub fn is_shutdown(&self) -> bool {
        self.kill
    }

    /// Awaits for the the event in receiver, if it get it then it sets [self.kill] = true
    pub async fn recv(&mut self) {
        if self.kill {
            return;
        }
        self.receiver.recv().await.unwrap();

        self.kill = true
    }
}
