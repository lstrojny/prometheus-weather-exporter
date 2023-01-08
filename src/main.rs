#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::missing_const_for_fn)]
#![warn(clippy::cargo)]
#![warn(clippy::cargo_common_metadata)]
#![allow(clippy::no_effect_underscore_binding)]
#![feature(absolute_path)]
extern crate core;

use crate::config::{get_provider_tasks, read, Credentials, ProviderTasks, DEFAULT_CONFIG};
use crate::http::{maybe_authenticate, ForbiddenResponse, UnauthorizedResponse};
use crate::prometheus::format;
use crate::providers::Weather;
use clap::{arg, command, Parser};
use log::{debug, error, info};
use rocket::tokio::task;
use rocket::tokio::task::JoinSet;
use rocket::{get, http::Status, launch, routes, Either, State};
use rocket_basicauth::BasicAuth;
use std::path::PathBuf;
use std::process::exit;
use tokio::task::JoinError;

mod config;
mod http;
mod prometheus;
mod providers;

#[get("/")]
#[allow(clippy::needless_pass_by_value)]
fn index(
    credentials: &State<Option<Credentials>>,
    auth: Option<BasicAuth>,
) -> Result<(Status, &'static str), Either<UnauthorizedResponse, ForbiddenResponse>> {
    match maybe_authenticate(credentials, &auth) {
        Ok(_) => Ok((Status::NotFound, "Check /metrics")),
        Err(e) => Err(e),
    }
}

#[get("/metrics")]
async fn metrics(
    unscheduled_tasks: &State<ProviderTasks>,
    credentials: &State<Option<Credentials>>,
    auth: Option<BasicAuth>,
) -> Result<(Status, String), Either<UnauthorizedResponse, ForbiddenResponse>> {
    match maybe_authenticate(credentials, &auth) {
        Ok(_) => Ok(serve_metrics(unscheduled_tasks).await),
        Err(e) => Err(e),
    }
}

async fn serve_metrics(unscheduled_tasks: &State<ProviderTasks>) -> (Status, String) {
    let mut join_set = JoinSet::new();

    #[allow(clippy::unnecessary_to_owned)]
    for (provider, req, cache) in unscheduled_tasks.to_vec() {
        let prov_req = req.clone();
        let task_cache = cache.clone();
        join_set.spawn(task::spawn_blocking(move || {
            info!(
                "Requesting weather data for {:?} from {:?} ({:?})",
                prov_req.name,
                provider.id(),
                prov_req.query,
            );
            provider.for_coordinates(&task_cache, &prov_req)
        }));
    }

    wait_for_metrics(join_set).await.map_or_else(
        |e| {
            error!("Error while fetching weather data {e}");
            (
                Status::InternalServerError,
                "Error while fetching weather data. Check the logs".to_string(),
            )
        },
        |metrics| (Status::Ok, metrics),
    )
}

async fn wait_for_metrics(
    mut join_set: JoinSet<Result<anyhow::Result<Weather>, JoinError>>,
) -> anyhow::Result<String> {
    let mut weather = vec![];

    while let Some(result) = join_set.join_next().await {
        weather.push(result???);
    }

    format(weather)
}

#[cfg(debug_assertions)]
#[derive(Copy, Clone, Debug, Default)]
pub struct DebugLevel;

#[cfg(debug_assertions)]
impl clap_verbosity_flag::LogLevel for DebugLevel {
    fn default() -> Option<log::Level> {
        Some(log::Level::Debug)
    }
}

#[cfg(debug_assertions)]
type DefaultLogLevel = DebugLevel;

#[cfg(not(debug_assertions))]
type DefaultLogLevel = clap_verbosity_flag::WarnLevel;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity<DefaultLogLevel>,

    // Custom config file location
    #[arg(short, long, default_value = DEFAULT_CONFIG)]
    config: PathBuf,
}

#[launch]
fn rocket() -> _ {
    let args = Args::parse();

    let log_level = args.verbose.log_level().unwrap();

    stderrlog::new()
        .verbosity(log_level)
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();

    debug!("Configured logger with level {log_level:?}");

    let config = read(args.config, log_level).unwrap_or_else(|e| {
        error!("Fatal error: {e}");
        exit(1);
    });

    let tasks = get_provider_tasks(config.clone()).unwrap_or_else(|e| {
        error!("Fatal error: {e}");
        exit(1);
    });

    rocket::custom(config.http)
        .manage(tasks)
        .manage(config.auth)
        .mount("/", routes![index, metrics])
}
