use std::process::exit;
use std::time::Duration;
use aes::cipher::{KeyIvInit, StreamCipher};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use futures::executor::block_on;
use meshtastic::Message;
use meshtastic::protobufs::mesh_packet::PayloadVariant;
use meshtastic::protobufs::{Data, MeshPacket, ServiceEnvelope};
use paho_mqtt::{Message as mqttMessage, MQTT_VERSION_5};
use sqlx::{Pool, Sqlite};
use std::sync::Mutex;
use tracing::{debug, error, info, warn};
use crate::database;

type Aes128Ctr32LE = ctr::Ctr32LE<aes::Aes128>;
type Aes256Ctr32LE = ctr::Ctr32LE<aes::Aes256>;

struct Msg {
    nodeid: u32,
    message: String,
}

static LAST_MSG: Mutex<Msg> = Mutex::new(Msg { nodeid: 0, message: String::new() });

pub async fn connect(db_pool: &Pool<Sqlite>, server_uri: String, username: String, password: String, topics: Vec<(String, String)>, qos: i32, webhook_url: String) {
    debug!("Creating initial client opts for server {}", server_uri);
    let create_opts = paho_mqtt::CreateOptionsBuilder::new()
        .server_uri(server_uri.clone())
        .client_id("mqtt_discord_relay")
        .finalize();

    debug!("[{}] Creating client", server_uri);
    let mut client = match paho_mqtt::AsyncClient::new(create_opts) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create mqtt client: {}", e);
            exit(1);
        }
    };

    debug!("[{}] Setting disconnect callback", server_uri);
    client.set_disconnected_callback(|_, _props, reason| {
        warn!("Client disconnected with reason: {}", reason);
    });


    if let Err(e) = block_on(async {
        debug!("[{}] Getting client stream", server_uri);
        let stream = client.get_stream(2048);

        debug!("[{}] Building connection opts", server_uri);
        let conn_opts = paho_mqtt::ConnectOptionsBuilder::with_mqtt_version(MQTT_VERSION_5)
            .clean_start(false)
            .properties(paho_mqtt::Properties::default())
            .user_name(username)
            .password(password)
            .finalize();

        debug!("[{}] Connecting to server", server_uri);
        if let Err(e) = client.connect(conn_opts).await {
            error!("Failed to connect to the MQTT server: {}", e);
        };

        let sub_topics = topics.iter().map(|f| {
            f.0.clone()
        })
            .collect::<Vec<String>>();

        debug!("[{}] Setting subscriber", server_uri);
        if let Err(e) = client.subscribe_many_same_qos(&sub_topics, qos)
            .await {
            error!("Failed to subscribe to topics on the MQTT server: {}", e);
        }

        debug!("[{}] Starting message loop", server_uri);
        info!("Connected to MQTT server: {}. Awaiting messages...", server_uri);
        while let Ok(msg_opt) = stream.recv().await {
            if let Some(msg) = msg_opt {
                on_message(db_pool, msg, &topics, &webhook_url).await;
            } else {
                warn!("[{}] Lost connection. Attempting reconnect.", server_uri);
                while let Err(e) = client.reconnect().await {
                    error!("Failed to reconnect client: {}", e);
                    async_std::task::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        Ok::<(), paho_mqtt::Error>(())
    }) {
        error!("{}", e);
    };
}

pub async fn on_message(db_pool: &Pool<Sqlite>, msg: mqttMessage, topics: &Vec<(String, String)>, webhook_url: &String) {
    info!("[{}] Received message", msg.topic());

    let topic = msg.topic();
    let parent_topic = format!("{}#", &topic[0..topic.len() - 9]);
    let key = topics.iter().find(|(f, _)| f == parent_topic.as_str());
    if let Some((_, key)) = key {
        debug!("[{}] Got key", topic);
        let (packet, subpacket) = match decrypt_message(msg.payload(), key.clone()) {
            Ok(m) => m,
            Err(e) => {
                error!("{}", e);
                return;
            }
        };

        match subpacket.portnum {
            // Text Message
            1 => {
                let text = match String::from_utf8(subpacket.payload) {
                    Ok(text) => text,
                    Err(e) => {
                        error!("Failed to parse subpacket text payload. {}", e);
                        return;
                    }
                };

                let name = match database::get_node_name(db_pool, packet.from).await {
                    Ok(n) => n,
                    Err(e) => {
                        error!("{}", e);
                        format!("{}", packet.from)
                    }
                };


                let mut last_msg = LAST_MSG.lock().unwrap();
                if last_msg.nodeid == packet.from && last_msg.message == text {
                    return;
                }

                last_msg.nodeid = packet.from;
                last_msg.message = text.clone();

                let full_discord_msg = format!("[{}] | {}: {}", parent_topic, name, text);
                crate::discord::send_message(webhook_url.clone(), full_discord_msg).await;
            },
            // NodeInfo
            4 => {
                let user = match meshtastic::protobufs::User::decode(subpacket.payload.as_slice()) {
                    Ok(u) => u,
                    Err(e) => {
                        error!("Failed to get NODEINFO_APP user proto: {}", e);
                        return;
                    }
                };

                if let Err(e) = database::add_node_name(db_pool, user.long_name, packet.from).await {
                    error!("{}", e);
                    return;
                };
            }
            _ => {}
        }
    } else {
        warn!("Failed to find key for parent topic: {}", parent_topic);
    }
}

pub fn decrypt_message(payload: &[u8], key: String) -> Result<(MeshPacket, Data), String> {
    debug!("Attempting to decrypt message");

    let envelope = match ServiceEnvelope::decode(payload) {
        Ok(e) => e,
        Err(e) => {
            return Err(format!("Failed to decode protobuf: {}", e));
        }
    };

    if let Some(packet) = envelope.packet {
        debug!("Packet {:?}", packet);
        let r_packet = packet.clone();
        if let Some(variant) = packet.payload_variant {
            match variant {
                PayloadVariant::Encrypted(data) => {
                    let node_id = packet.from;
                    let packet_id = packet.id;
                    let mut nonce_bytes = [0u8; 16];
                    nonce_bytes[0..4].copy_from_slice(&packet_id.to_le_bytes());
                    nonce_bytes[8..12].copy_from_slice(&node_id.to_le_bytes());
                    debug!("Calculated nonce: {:?}", nonce_bytes);

                    let key_bytes = match BASE64_STANDARD.decode(key) {
                        Ok(k) => k,
                        Err(e) => {
                            return Err(format!("Failed to decode channel key: {}", e));
                        }
                    };

                    debug!("Key Length: {}", key_bytes.len());

                    let mut msg = vec![0u8; data.len()];
                    if key_bytes.len() == 16 {
                        let mut cipher = Aes128Ctr32LE::new(key_bytes.as_slice().into(), nonce_bytes.as_slice().into());
                        if let Err(e) = cipher.apply_keystream_b2b(&data, &mut msg) {
                            return Err(format!("Failed to encrypt message: {}", e));
                        };
                    } else if key_bytes.len() == 32 {
                        let mut cipher = Aes256Ctr32LE::new(key_bytes.as_slice().into(), nonce_bytes.as_slice().into());
                        if let Err(e) = cipher.apply_keystream_b2b(&data, &mut msg) {
                            return Err(format!("Failed to encrypt message: {}", e));
                        }
                    } else {
                        return Err("Invalid key length.".to_string());
                    }

                    let subpacket = match meshtastic::protobufs::Data::decode(&mut msg.as_slice()){
                        Ok(m) => m,
                        Err(e) => {
                            return Err(format!("Failed to decrypt protobuf message: {}", e));
                        }
                    };

                    debug!("{:?}", subpacket);

                    return Ok((r_packet, subpacket));
                },
                PayloadVariant::Decoded(_) => {
                    return Err("Not implemented yet.".to_string());
                }
            }
        } else {
            return Err("Failed to decode payload variant".to_string());
        }
    } else {
        return Err("Failed to get packet".to_string());
    }
}