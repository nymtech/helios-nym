use crate::error::BackendError;
use futures::SinkExt;
use nym_socks5::client::{Socks5ControlMessage, Socks5ControlMessageSender};
use tokio::task::JoinHandle;

pub mod config;
pub mod error;
pub mod tasks;

pub struct Socks5Server {
    control_tx: Socks5ControlMessageSender,
    exit_join_handler: JoinHandle<()>,
    _msg_rx: task::StatusReceiver,
}

impl Socks5Server {
    pub fn new(
        control_tx: Socks5ControlMessageSender,
        exit_join_handler: JoinHandle<()>,
        _msg_rx: task::StatusReceiver,
    ) -> Self {
        Socks5Server {
            control_tx,
            exit_join_handler,
            _msg_rx,
        }
    }

    pub async fn terminate(mut self) {
        // disconnect
        match self
            .control_tx
            .send(Socks5ControlMessage::Stop)
            .await
            .map_err(|err| {
                log::warn!("Failed trying to send disconnect signal: {err}");
                BackendError::CoundNotSendDisconnectSignal
            }) {
            Ok(_) => log::info!("✅✅✅✅✅✅ SOCKS5 >>> Disconnected"),
            Err(e) => log::error!("Failed to disconnect SOCKS5: {}", e),
        }

        if let Err(e) = self.exit_join_handler.await {
            log::error!("Failed to join after existing SOCKS5: {:?}", e);
        }
    }
}
