use crate::authentication::CredentialsStore;
use crate::providers::units::Coordinates;
use crate::providers::HttpRequestCache;
use crate::providers::{Providers, WeatherProvider, WeatherRequest};
use anyhow::{anyhow, Context};
use const_format::concatcp;
use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use log::{debug, info, warn, Level};
use moka::sync::CacheBuilder;
use reqwest::blocking::Client;
use rocket::config::Ident;
use rocket::figment::providers::Serialized;
use rocket::log::LogLevel as RocketLogLevel;
use rocket::serde::Serialize;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_CONFIG: &str = concatcp!("/etc/", NAME, "/weathermen.toml");
const DEFAULT_PORT: u16 = 36333;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Location {
    pub name: Option<String>,
    #[serde(flatten)]
    pub coordinates: Coordinates,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    #[serde(rename = "location")]
    pub locations: BTreeMap<String, Location>,
    #[serde(rename = "provider")]
    pub providers: Option<Providers>,
    pub http: rocket::Config,
    pub auth: Option<CredentialsStore>,
}

fn default_rocket_config() -> rocket::Config {
    rocket::Config {
        port: DEFAULT_PORT,
        ident: Ident::try_new(NAME.to_owned()).expect("Hardcoded, cannot fail"),
        ..rocket::Config::default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            locations: BTreeMap::new(),
            providers: None,
            http: default_rocket_config(),
            auth: None,
        }
    }
}

pub fn read(config_file: PathBuf, log_level: Option<Level>) -> anyhow::Result<Config> {
    info!("Reading config file {:?}", config_file);

    let config: Config = Figment::new()
        .merge(Serialized::defaults(Config::default()))
        .merge(Toml::file(config_file))
        .merge((
            "http.log_level",
            match log_level {
                Some(Level::Trace | Level::Debug) => RocketLogLevel::Debug,
                Some(Level::Info | Level::Warn) => RocketLogLevel::Normal,
                Some(Level::Error) => RocketLogLevel::Critical,
                None => RocketLogLevel::Off,
            },
        ))
        .merge(Env::prefixed("PROMW_").split("__"))
        .extract()?;

    debug!("Read config is {:?}", config);

    Ok(config)
}

pub type ProviderTasks = Vec<Task>;

#[derive(Clone)]
pub struct Task {
    pub provider: Arc<dyn WeatherProvider + Send + Sync>,
    pub request: WeatherRequest<Coordinates>,
    pub client: Client,
    pub cache: HttpRequestCache,
}

pub fn get_provider_tasks(config: Config) -> anyhow::Result<ProviderTasks> {
    let configured_providers = config
        .providers
        .with_context(|| "No providers configured")?;

    let mut tasks: ProviderTasks = vec![];

    for configured_provider in configured_providers {
        let max_capacity = config
            .locations
            .len()
            .checked_mul(configured_provider.cache_cardinality())
            .ok_or_else(|| anyhow!("Overflow while calculating max capacity"))?;
        let cache = CacheBuilder::new(max_capacity.try_into()?)
            .time_to_live(configured_provider.refresh_interval())
            .build();

        debug!("Found configured provider {configured_provider:?}");

        if configured_provider.refresh_interval() < Duration::from_secs(60 * 5) {
            warn!(
                "Updating weather information more often than every 5 minutes is discouraged. Consider increasing the refresh interval for {}",
                configured_provider.id()
            );
        }

        let locations = config.locations.clone();
        for (name, location) in locations {
            tasks.push(Task {
                provider: Arc::clone(&configured_provider),
                request: WeatherRequest {
                    name: location.name.unwrap_or(name),
                    query: location.coordinates,
                },
                client: Client::new(),
                cache: cache.clone(),
            });
        }
    }

    Ok(tasks)
}
