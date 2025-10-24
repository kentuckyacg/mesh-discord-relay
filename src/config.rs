#![allow(dead_code)]

use serde::Deserialize;


#[derive(Deserialize, Debug)]
pub struct Config {
    pub base: Base,
    pub mqtt: MQTT
}

#[derive(Deserialize, Debug)]
pub struct Base {
    pub webhook: String,
    pub database: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MQTT {
    pub uri: String,
    pub username: String,
    pub password: String,
    pub channels: Vec<Channel>
}


#[derive(Deserialize, Debug, Clone)]
pub struct Channel {
    pub topic: String,
    pub key: String
}

pub fn read_config(config_file: String) -> Result<Config, String> {
    let config_str = match std::fs::read_to_string(config_file) {
        Ok(c) => c,
        Err(e) => return Err(format!("Error reading config: {}", e))
    };

    let config = match toml::from_str(config_str.as_str()) {
        Ok(c) => c,
        Err(e) => return Err(format!("Error parsing config: {}", e))
    };

    Ok(config)
}