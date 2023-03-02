// Copyright 2023 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use crate::error::{BackendError, Result};
use client_core::config::Config as BaseConfig;
use nym_socks5::client::config::Config as Socks5Config;

static SOCKS5_CONFIG_ID: &str = "nym-harbour-master";

pub fn socks5_config_id_appended_with(gateway_id: &str) -> Result<String> {
    use std::fmt::Write as _;
    let mut id = SOCKS5_CONFIG_ID.to_string();
    write!(id, "-{}", gateway_id)?;
    Ok(id)
}

#[derive(Debug)]
pub struct Config {
    socks5: Socks5Config,
}

impl Config {
    pub fn new<S: Into<String>>(id: S, provider_mix_address: S) -> Self {
        Config {
            socks5: Socks5Config::new(id, provider_mix_address),
        }
    }

    pub fn get_base(&self) -> &BaseConfig<Socks5Config> {
        self.socks5.get_base()
    }

    pub fn get_base_mut(&mut self) -> &mut BaseConfig<Socks5Config> {
        self.socks5.get_base_mut()
    }

    pub async fn init(id: &str, service_provider: &str) -> Result<()> {
        // use mainnet
        network_defaults::setup_env(None);

        log::info!("Initialising...");

        let id = id.to_owned();
        let service_provider = service_provider.to_owned();

        // The client initialization was originally not written for this use case, so there are
        // lots of ways it can panic. Until we have proper error handling in the init code for the
        // clients we'll catch any panics here by spawning a new runtime in a separate thread.
        std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .expect("Failed to create tokio runtime")
                .block_on(async move { init_socks5_config(id, service_provider).await })
        })
        .join()
        .map_err(|_| BackendError::InitializationPanic)??;

        log::info!("Configuration saved 🚀");
        Ok(())
    }
}

pub async fn init_socks5_config(id: String, provider_address: String) -> Result<()> {
    let mut config = Config::new(&id, &provider_address);

    if let Ok(raw_validators) = std::env::var(config_common::defaults::var_names::NYM_API) {
        config
            .get_base_mut()
            .set_custom_nym_apis(config_common::parse_urls(&raw_validators));
    }

    let gateway = client_core::init::setup_gateway_from_config::<Socks5Config, _>(
        true,
        None,
        config.get_base(),
        false,
    )
    .await?;

    config.get_base_mut().set_gateway_endpoint(gateway);
    config.get_base_mut().set_no_cover_traffic();

    let address = client_core::init::get_client_address_from_stored_keys(config.get_base())?;
    log::info!("The address of this client is: {}", address);
    Ok(())
}
