// SPDX-FileCopyrightText: 2023 perillamint <perillamint@silicon.moe>
// SPDX-FileCopyrightText: 2020-2022 Alex Grinman <me@alexgr.in>
//
// SPDX-License-Identifier: MIT

use std::net::{SocketAddr, ToSocketAddrs};

use super::*;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::openid2::{authorize, fetch_token};

const SETTINGS_DIR: &str = ".portalgun";
const SECRET_KEY_FILE: &str = "auth.json";

/// Command line arguments
#[derive(Debug, Parser)]
#[command(name = "portalgun")]
#[command(author = "perillamint, tunnelto")]
#[command(about = "Expose your local web server to the internet with a public url.", long_about = None)]
struct Opts {
    /// A level of verbosity, and can be used multiple times
    #[clap(long, short = 'v')]
    verbose: bool,

    #[command(subcommand)]
    command: Option<SubCommand>,

    /// Specify a sub-domain for this tunnel
    #[clap(long, short = 's')]
    sub_domain: Option<String>,

    /// Sets the HOST (i.e. localhost) to forward incoming tunnel traffic to
    #[clap(long = "host", default_value = "localhost")]
    local_host: String,

    /// Sets the protocol for local forwarding (i.e. https://localhost) to forward incoming tunnel traffic to
    #[clap(long = "use-tls", short = 't')]
    use_tls: bool,

    /// Sets the port to forward incoming tunnel traffic to on the target host
    #[clap(long = "port", default_value = "8000")]
    port: u16,

    /// Sets the address of the local introspection dashboard
    #[clap(long = "dashboard-port")]
    dashboard_port: Option<u16>,
}

#[derive(Debug, Subcommand)]
enum SubCommand {
    /// Login using OpenID Connect. This will store the authentication token on disk for future use.
    Login {
        /// OpenID Discovery URL to use for authentication
        #[clap(long = "oidc")]
        oidc: String,
        /// OpenID Client ID to use for authentication
        #[clap(long = "client-id")]
        client_id: String,
        /// OpenID scopes. separated in comma
        #[clap(long = "scopes", default_value = "openid,portalgun")]
        scopes: String,
        /// Corresponding tunnel server websocket URL, example: wss://tunnel.example.com
        #[clap(long = "control-server")]
        control_server: Url,
    },
}

/// Config
#[derive(Debug, Clone)]
pub struct Config {
    pub client_id: ClientId,
    pub control_url: Url,
    pub use_tls: bool,
    pub local_host: String,
    pub local_port: u16,
    pub local_addr: SocketAddr,
    pub sub_domain: Option<String>,
    pub secret_key: Option<SecretKey>,
    pub first_run: bool,
    pub dashboard_port: u16,
    pub verbose: bool,
}

/// Auth storage
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthStorage {
    oidc: String,
    client_id: String,
    refresh_token: String,
    control_server: Url,
}

impl Config {
    /// Parse the URL to use to connect to the wormhole control server
    pub async fn get() -> Result<Config, ()> {
        // parse the opts
        let opts: Opts = Opts::parse();

        if opts.verbose {
            std::env::set_var("RUST_LOG", "tunnelto=debug");
        }

        pretty_env_logger::init();

        let (secret_key, sub_domain, control_url) = match opts.command {
            Some(SubCommand::Login {
                oidc,
                client_id,
                scopes,
                control_server,
            }) => {
                let control_url = control_server.join("wormhole").expect("Malformed URL");
                let scope_vec: Vec<String> = scopes.split(',').map(|str| str.to_owned()).collect();
                let refresh = authorize(&oidc, &client_id, scope_vec).await.unwrap();

                let auth_storage = AuthStorage {
                    oidc,
                    client_id,
                    refresh_token: refresh,
                    control_server: control_url,
                };

                let auth_json =
                    serde_json::to_string(&auth_storage).expect("Failed to serialize credential.");

                let settings_dir = match dirs::home_dir().map(|h| h.join(SETTINGS_DIR)) {
                    Some(path) => path,
                    None => {
                        panic!("Could not find home directory to store token.")
                    }
                };
                std::fs::create_dir_all(&settings_dir)
                    .expect("Fail to create file in home directory");
                std::fs::write(settings_dir.join(SECRET_KEY_FILE), auth_json)
                    .expect("Failed to store credential.");

                eprintln!("Authentication key stored successfully!");
                std::process::exit(0);
            }
            None => {
                let auth_file = dirs::home_dir()
                    .map(|h| h.join(SETTINGS_DIR).join(SECRET_KEY_FILE))
                    .expect("Failed to access home directory.");
                let auth_json = if auth_file.exists() {
                    std::fs::read_to_string(auth_file.clone())
                        .map_err(|e| error!("Error reading credential: {:?}", e))
                        .unwrap()
                } else {
                    eprintln!("Credential file not found. Please login first.");
                    std::process::exit(1);
                };

                let mut credential: AuthStorage =
                    serde_json::from_str(&auth_json).expect("Failed to deserialize credential.");

                let (access_token, refresh_token) = fetch_token(
                    &credential.oidc,
                    &credential.client_id,
                    &credential.refresh_token,
                )
                .await
                .expect("Failed to refresh session.");

                if let Some(refresh) = refresh_token {
                    credential.refresh_token = refresh;
                    let json = serde_json::to_string(&credential)
                        .expect("Failed to serialize credential.");
                    std::fs::write(auth_file, json).expect("Failed to store credential.");
                }

                (access_token, opts.sub_domain, credential.control_server)
            }
        };

        let local_addr = match (opts.local_host.as_str(), opts.port)
            .to_socket_addrs()
            .unwrap_or(vec![].into_iter())
            .next()
        {
            Some(addr) => addr,
            None => {
                error!(
                    "An invalid local address was specified: {}:{}",
                    opts.local_host.as_str(),
                    opts.port
                );
                return Err(());
            }
        };

        Ok(Config {
            client_id: ClientId::generate(),
            local_host: opts.local_host,
            use_tls: opts.use_tls,
            control_url,
            local_port: opts.port,
            local_addr,
            sub_domain,
            dashboard_port: opts.dashboard_port.unwrap_or(0),
            verbose: opts.verbose,
            secret_key: Some(secret_key).map(SecretKey),
            first_run: true,
        })
    }

    pub fn activation_url(&self, full_hostname: &str) -> String {
        format!(
            "{}://{}",
            if self.control_url.scheme() == "ws" {
                "http"
            } else {
                "https"
            },
            full_hostname
        )
    }

    pub fn forward_url(&self) -> String {
        let scheme = if self.use_tls { "https" } else { "http" };
        format!("{}://{}:{}", &scheme, &self.local_host, &self.local_port)
    }
    pub fn ws_forward_url(&self) -> String {
        let scheme = if self.use_tls { "wss" } else { "ws" };
        format!("{}://{}:{}", scheme, &self.local_host, &self.local_port)
    }
}
