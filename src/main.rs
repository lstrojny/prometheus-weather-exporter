#![cfg_attr(feature = "nightly", feature(test))]

use crate::config::{read, DEFAULT_CONFIG};
use crate::error::exit_if_handle_fatal;
use clap::{arg, command, Parser};
use rocket::{launch, Build, Rocket};
use std::path::PathBuf;

mod authentication;
mod config;
mod error;
mod http_server;
mod logging;
mod prometheus;
mod providers;

#[cfg(debug_assertions)]
type DefaultLogLevel = clap_verbosity_flag::DebugLevel;

#[cfg(not(debug_assertions))]
type DefaultLogLevel = clap_verbosity_flag::ErrorLevel;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity<DefaultLogLevel>,

    // Custom config file location
    #[arg(short, long, default_value = DEFAULT_CONFIG)]
    config: PathBuf,
}

/// Start the HTTP server to serve Prometheus metrics
///
/// # Panics
///
/// Will panic if the log level cannot be parsed
#[launch]
async fn start_server() -> Rocket<Build> {
    let args = Args::parse();

    let log_level = args.verbose.log_level();

    if let Some(level) = log_level {
        logging::init(level).expect("Logging successfully initialized");
    }

    let config = read(args.config, log_level).unwrap_or_else(exit_if_handle_fatal);

    http_server::configure_rocket(config).await
}
