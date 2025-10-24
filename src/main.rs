use clap::Parser;
use tracing::{debug, info};

mod logging;
mod mqtt;
mod discord;
mod args;
mod config;
mod database;

#[tokio::main]
async fn main() -> Result<(), String> {
    // Parse arguments
    let args = args::Args::parse();

    // Initialize tracing subscriber
    logging::init(args.verbose, args.debug);
    debug!("Running with arguments: \n{:#?}", args);

    // Read in the config
    let config_file = args.config_file.unwrap_or("./config.toml".to_string());
    let config = match config::read_config(config_file) {
        Ok(config) => config,
        Err(e) => {
            return Err(e);
        }
    };

    debug!("Using config: {:#?}", config);

    debug!("Setting up database");
    let db_pool = match database::init(config.base.database).await {
        Ok(db_pool) => db_pool,
        Err(e) => {
            return Err(e);
        }
    };

    debug!("Formating channels for mqtt");
    let mut channels = Vec::new();
    for channel in config.mqtt.channels {
        channels.push((channel.topic, channel.key));
    }

    info!("Finished initializing. Starting connection to MQTT.");
    mqtt::connect(
        &db_pool,
        config.mqtt.uri,
        config.mqtt.username,
        config.mqtt.password,
        channels,
        1,
        config.base.webhook,
    ).await;

    Ok(())
}
