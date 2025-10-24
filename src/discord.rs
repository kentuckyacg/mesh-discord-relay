use std::time::Duration;
use reqwest::Client;
use tracing::{debug, error, info};
use serde_json::json;

pub async fn send_message(webhook_url: String, message: String) {
    debug!("Sending discord message: {}", message);

    let client = Client::new();
    let payload = json!({
        "content": message,
        "username": "Meshtastic Relay",
    });

    let res = client.post(webhook_url)
        .json(&payload)
        .timeout(Duration::from_secs(5))
        .send()
        .await;

    if let Err(err) = res {
        error!("Failed to send message: {}", err);
        return;
    }

    info!("Sent message to discord.");
}