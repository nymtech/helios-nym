// Copyright 2023 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use tap::TapFallible;
use tokio::task::JoinHandle;

use client_core::config::{ClientCoreConfigTrait, GatewayEndpointConfig};
use config_common::NymConfig;
use futures::channel::mpsc;
use nym_socks5::client::NymClient as Socks5NymClient;
use nym_socks5::client::{config::Config as Socks5Config, Socks5ControlMessageSender};

use crate::error::Result;

pub type ExitStatusReceiver = futures::channel::oneshot::Receiver<Socks5ExitStatusMessage>;

/// Status messages sent by the SOCKS5 client task to the main tauri task.
#[derive(Debug)]
pub enum Socks5ExitStatusMessage {
    /// The SOCKS5 task successfully stopped
    Stopped,
    /// The SOCKS5 task failed to start
    Failed(Box<dyn std::error::Error + Send>),
}

/// The main SOCKS5 client task. It loads the configuration from file determined by the `id`.
pub fn start_nym_socks5_client(
    id: &str,
) -> Result<(
    Socks5ControlMessageSender,
    task::StatusReceiver,
    ExitStatusReceiver,
    GatewayEndpointConfig,
)> {
    log::info!("Loading config from file: {id}");
    let config = Socks5Config::load_from_file(id)
        .tap_err(|_| log::warn!("Failed to load configuration file"))?;
    let used_gateway = config.get_base().get_gateway_endpoint().clone();

    let socks5_client = Socks5NymClient::new(config);
    log::info!("Starting socks5 client");

    // Channel to send control messages to the socks5 client
    let (socks5_ctrl_tx, socks5_ctrl_rx) = mpsc::unbounded();

    // Channel to send status update messages from the background socks5 task to the frontend.
    let (socks5_status_tx, socks5_status_rx) = mpsc::channel(128);

    // Channel to signal back to the main task when the socks5 client finishes, and why
    let (socks5_exit_tx, socks5_exit_rx) = futures::channel::oneshot::channel();

    // Spawn a separate runtime for the socks5 client so we can forcefully terminate.
    // Once we can gracefully shutdown the socks5 client we can get rid of this.
    // The status channel is used to both get the state of the task, and if it's closed, to check
    // for panic.
    std::thread::spawn(|| {
        let result = tokio::runtime::Runtime::new()
            .expect("Failed to create runtime for SOCKS5 client")
            .block_on(async move {
                socks5_client
                    .run_and_listen(socks5_ctrl_rx, socks5_status_tx)
                    .await
            });

        if let Err(err) = result {
            log::error!("SOCKS5 proxy failed: {err}");
            socks5_exit_tx
                .send(Socks5ExitStatusMessage::Failed(err))
                .expect("Failed to send status message back to main task");
            return;
        }

        log::info!("SOCKS5 task finished");
        socks5_exit_tx
            .send(Socks5ExitStatusMessage::Stopped)
            .expect("Failed to send status message back to main task");
    });

    Ok((
        socks5_ctrl_tx,
        socks5_status_rx,
        socks5_exit_rx,
        used_gateway,
    ))
}

pub fn start_disconnect_listener(exit_status_receiver: ExitStatusReceiver) -> JoinHandle<()> {
    log::trace!("Starting disconnect listener");
    tokio::spawn(async move {
        match exit_status_receiver.await {
            Ok(Socks5ExitStatusMessage::Stopped) => {
                log::info!("SOCKS5 task reported it has finished");
            }
            Ok(Socks5ExitStatusMessage::Failed(err)) => {
                log::info!("SOCKS5 task reported error: {}", err);
            }
            Err(_) => {
                log::info!("SOCKS5 task appears to have stopped abruptly");
            }
        }
    })
}
