use tokio::sync::broadcast::Receiver;
use log::info;

pub struct Shutdown {
    kill: bool,
    receiver: Receiver<()>,
    caller: String
}

impl Shutdown {
    pub fn new(receiver: Receiver<()>, caller: String) -> Self {
        Self {
            receiver,
            caller,
            kill: false
        }
    }

    pub fn is_shutdown(&self) -> bool {
        self.kill
    }

    pub async fn recv(&mut self) {
        if self.kill {
            return
        }

        let _ = self.receiver.recv().await.unwrap();
        info!(target: "shutdown", "Received kill signal at: {}", self.caller);

        self.kill = true
    }
}
