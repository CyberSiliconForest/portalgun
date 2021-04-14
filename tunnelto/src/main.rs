use futures::channel::mpsc::{unbounded, UnboundedSender};
use futures::{SinkExt, StreamExt};

use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use human_panic::setup_panic;
pub use log::{debug, error, info, warn};

use std::collections::HashMap;
use std::env;
use std::sync::{Arc, RwLock};

mod config;
mod error;
mod introspect;
mod local;
mod spinner;
pub use self::error::*;

pub use config::*;
pub use tunnelto_lib::*;

use crate::introspect::IntrospectionAddrs;
use colored::Colorize;
use std::time::Duration;

pub type ActiveStreams = Arc<RwLock<HashMap<StreamId, UnboundedSender<StreamMessage>>>>;

lazy_static::lazy_static! {
    pub static ref ACTIVE_STREAMS:ActiveStreams = Arc::new(RwLock::new(HashMap::new()));
}

#[derive(Debug, Clone)]
pub enum StreamMessage {
    Data(Vec<u8>),
    Close,
}

#[tokio::main]
async fn main() {
    setup_panic!();

    let mut config = match Config::get() {
        Ok(config) => config,
        Err(_) => return,
    };

    let introspect_addrs = introspect::start_introspection_server(config.clone());

    loop {
        let (restart_tx, mut restart_rx) = unbounded();
        let wormhole = run_wormhole(config.clone(), introspect_addrs.clone(), restart_tx);
        let result = futures::future::select(Box::pin(wormhole), restart_rx.next()).await;

        match result {
            futures::future::Either::Left((wormhole_result, _)) => {
                if let Err(e) = wormhole_result {
                    debug!("wormhole error: {:?}", e);
                    match e {
                        Error::WebSocketError(err) => {
                            warn!("websocket error: {:?}..restarting", err);
                            continue;
                        }
                        _ => {}
                    };
                    eprintln!("Error: {}", format!("{}", e).red());
                    return;
                }
            }
            _ => {}
        };

        config.first_run = false;
        info!("restarting wormhole");
    }
}

/// Setup the tunnel to our control server
async fn run_wormhole(
    config: Config,
    introspect: IntrospectionAddrs,
    mut restart_tx: UnboundedSender<()>,
) -> Result<(), Error> {
    let websocket = connect_to_wormhole(&config).await?;

    if config.first_run {
        eprintln!(
            "Local Inspect Dashboard: {}{}",
            "http://localhost:".yellow(),
            introspect.web_explorer_address.port()
        );
    }

    // split reading and writing
    let (mut ws_sink, mut ws_stream) = websocket.split();

    // tunnel channel
    let (tunnel_tx, mut tunnel_rx) = unbounded::<ControlPacket>();

    // continuously write to websocket tunnel
    tokio::spawn(async move {
        loop {
            let packet = match tunnel_rx.next().await {
                Some(data) => data,
                None => {
                    warn!("control flow didn't send anything!");
                    let _ = restart_tx.send(()).await;
                    return;
                }
            };

            if let Err(e) = ws_sink.send(Message::binary(packet.serialize())).await {
                warn!("failed to write message to tunnel websocket: {:?}", e);
                let _ = restart_tx.send(()).await;
                return;
            }
        }
    });

    // continuously read from websocket tunnel

    loop {
        match ws_stream.next().await {
            Some(Ok(message)) => {
                match process_control_flow_message(
                    &introspect,
                    tunnel_tx.clone(),
                    message.into_data(),
                )
                .await
                {
                    Ok(packet) => {
                        debug!("Processed packet: {:?}", packet);
                    }
                    Err(e) => {
                        error!("Malformed protocol control packet: {:?}", e);
                        return Ok(());
                    }
                }
            }
            Some(Err(e)) => {
                warn!("websocket read error: {:?}", e);
                return Ok(());
            }
            None => {
                warn!("websocket sent none");
                return Ok(());
            }
        }
    }
}

async fn connect_to_wormhole(
    config: &Config,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, Error> {
    let spinner = if config.first_run {
        eprintln!(
            "{}\n\n",
            format!("{}", include_str!("../static/img/wormhole_ascii.txt")).green()
        );
        Some(spinner::new_spinner(
            "initializing remote tunnel, please stand by",
        ))
    } else {
        None
    };

    let (mut websocket, _) = tokio_tungstenite::connect_async(&config.control_url).await?;

    // send our Client Hello message
    let typ = match config.secret_key.clone() {
        Some(secret_key) => ClientType::Auth { key: secret_key },
        None => ClientType::Anonymous,
    };

    let client_hello = ClientHello::generate(config.sub_domain.clone(), typ);

    info!("connecting to wormhole as client {}", &client_hello.id);

    let hello = serde_json::to_vec(&client_hello).unwrap();
    websocket
        .send(Message::binary(hello))
        .await
        .expect("Failed to send client hello to wormhole server.");

    // wait for Server hello
    let server_hello_data = websocket
        .next()
        .await
        .ok_or(Error::NoResponseFromServer)??
        .into_data();
    let server_hello = serde_json::from_slice::<ServerHello>(&server_hello_data).map_err(|e| {
        error!("Couldn't parse server_hello from {:?}", e);
        Error::ServerReplyInvalid
    })?;

    let sub_domain = match server_hello {
        ServerHello::Success { sub_domain } => {
            info!("Server accepted our connection.");
            sub_domain
        }
        ServerHello::AuthFailed => {
            return Err(Error::AuthenticationFailed);
        }
        ServerHello::InvalidSubDomain => {
            return Err(Error::InvalidSubDomain);
        }
        ServerHello::SubDomainInUse => {
            return Err(Error::SubDomainInUse);
        }
    };

    // either first run or the tunnel changed domains
    // Note: the latter should rarely occur.
    if config.first_run || config.sub_domain.as_ref() != Some(&sub_domain) {
        if let Some(pb) = spinner {
            pb.finish_with_message(&format!(
                "Success! Remote tunnel created on: {}",
                &config.activation_url(&sub_domain).bold().green()
            ));
        }

        if config.sub_domain.is_some() && (config.sub_domain.as_ref() != Some(&sub_domain)) {
            eprintln!("{}",
                      ">>> Notice: to access the full sub-domain feature, get your a free authentication key at https://dashboard.tunnelto.dev.".yellow());
        }

        let p = match (config.scheme.as_str(), config.local_port.as_ref()) {
            (_, Some(p)) => format!(":{}", p),
            ("http", None) => ":8000".to_string(),
            (_, _) => "".to_string(),
        };

        eprintln!(
            "{} Forwarding to {}://{}{}\n",
            "=>".green(),
            config.scheme,
            config.local_host,
            p.yellow()
        );
    }

    Ok(websocket)
}

async fn process_control_flow_message(
    introspect: &IntrospectionAddrs,
    mut tunnel_tx: UnboundedSender<ControlPacket>,
    payload: Vec<u8>,
) -> Result<ControlPacket, Box<dyn std::error::Error>> {
    let control_packet = ControlPacket::deserialize(&payload)?;

    match &control_packet {
        ControlPacket::Init(stream_id) => {
            info!("stream[{:?}] -> init", stream_id.to_string());
        }
        ControlPacket::Ping => {
            log::info!("got ping");
            let _ = tunnel_tx.send(ControlPacket::Ping).await;
        }
        ControlPacket::Refused(_) => return Err("unexpected control packet".into()),
        ControlPacket::End(stream_id) => {
            // find the stream
            let stream_id = stream_id.clone();

            info!("got end stream [{:?}]", &stream_id);

            tokio::spawn(async move {
                let stream = ACTIVE_STREAMS.read().unwrap().get(&stream_id).cloned();
                if let Some(mut tx) = stream {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    let _ = tx.send(StreamMessage::Close).await.map_err(|e| {
                        error!("failed to send stream close: {:?}", e);
                    });
                    ACTIVE_STREAMS.write().unwrap().remove(&stream_id);
                }
            });
        }
        ControlPacket::Data(stream_id, data) => {
            info!(
                "stream[{:?}] -> new data: {:?}",
                stream_id.to_string(),
                data.len()
            );

            if !ACTIVE_STREAMS.read().unwrap().contains_key(&stream_id) {
                local::setup_new_stream(
                    introspect.forward_address.port(),
                    tunnel_tx.clone(),
                    stream_id.clone(),
                )
                .await;
            }

            // find the right stream
            let active_stream = ACTIVE_STREAMS.read().unwrap().get(&stream_id).cloned();

            // forward data to it
            if let Some(mut tx) = active_stream {
                tx.send(StreamMessage::Data(data.clone())).await?;
                info!("forwarded to local tcp ({})", stream_id.to_string());
            } else {
                error!("got data but no stream to send it to.");
                let _ = tunnel_tx
                    .send(ControlPacket::Refused(stream_id.clone()))
                    .await?;
            }
        }
    };

    Ok(control_packet.clone())
}
