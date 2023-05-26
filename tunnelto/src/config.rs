// SPDX-FileCopyrightText: 2023 perillamint <perillamint@silicon.moe>
// SPDX-FileCopyrightText: 2020-2022 Alex Grinman <me@alexgr.in>
//
// SPDX-License-Identifier: MIT

use std::net::{SocketAddr, ToSocketAddrs};

use super::*;
use clap::{Parser, Subcommand};

const HOST_ENV: &str = "CTRL_HOST";
const PORT_ENV: &str = "CTRL_PORT";
const TLS_OFF_ENV: &str = "CTRL_TLS_OFF";

const DEFAULT_HOST: &str = "tunnelto.dev";
const DEFAULT_CONTROL_HOST: &str = "wormhole.tunnelto.dev";
const DEFAULT_CONTROL_PORT: &str = "10001";

const SETTINGS_DIR: &str = ".tunnelto";
const SECRET_KEY_FILE: &str = "key.token";

/// Command line arguments
#[derive(Debug, Parser)]
#[command(name = "tunnelto")]
#[command(author = "TunnelTo <support@tunnelto.dev>")]
#[command(about = "Expose your local web server to the internet with a public url.", long_about = None)]
struct Opts {
    /// A level of verbosity, and can be used multiple times
    #[clap(long, short = 'v')]
    verbose: bool,

    #[command(subcommand)]
    command: Option<SubCommand>,

    /// Sets an API authentication key to use for this tunnel
    #[clap(long, short = 'k')]
    key: Option<String>,

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
        /// Corresponding tunnel server
        #[clap(long = "control-host")]
        control_host: String,
    },
}

/// Config
#[derive(Debug, Clone)]
pub struct Config {
    pub client_id: ClientId,
    pub control_url: String,
    pub use_tls: bool,
    pub host: String,
    pub local_host: String,
    pub local_port: u16,
    pub local_addr: SocketAddr,
    pub sub_domain: Option<String>,
    pub secret_key: Option<SecretKey>,
    pub control_tls_off: bool,
    pub first_run: bool,
    pub dashboard_port: u16,
    pub verbose: bool,
}

impl Config {
    /// Parse the URL to use to connect to the wormhole control server
    pub fn get() -> Result<Config, ()> {
        // parse the opts
        let opts: Opts = Opts::parse();

        if opts.verbose {
            std::env::set_var("RUST_LOG", "tunnelto=debug");
        }

        pretty_env_logger::init();

        let (secret_key, sub_domain) = match opts.command {
            Some(SubCommand::Login{oidc, client_id, control_host}) => {
                //let key = opts.key.unwrap_or(key);
                //let settings_dir = match dirs::home_dir().map(|h| h.join(SETTINGS_DIR)) {
                //    Some(path) => path,
                //    None => {
                //        panic!("Could not find home directory to store token.")
                //    }
                //};
                //std::fs::create_dir_all(&settings_dir)
                //    .expect("Fail to create file in home directory");
                //std::fs::write(settings_dir.join(SECRET_KEY_FILE), key)
                //    .expect("Failed to save authentication key file.");

                //eprintln!("Authentication key stored successfully!");
                std::process::exit(0);
            }
            None => {
                let key = opts.key;
                let sub_domain = opts.sub_domain;
                (
                    match key {
                        Some(key) => Some(key),
                        None => dirs::home_dir()
                            .map(|h| h.join(SETTINGS_DIR).join(SECRET_KEY_FILE))
                            .map(|path| {
                                if path.exists() {
                                    std::fs::read_to_string(path)
                                        .map_err(|e| {
                                            error!("Error reading authentication token: {:?}", e)
                                        })
                                        .ok()
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(None),
                    },
                    sub_domain,
                )
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

        // get the host url
        let tls_off = env::var(TLS_OFF_ENV).is_ok();
        let host = env::var(HOST_ENV).unwrap_or(DEFAULT_HOST.to_string());

        let control_host = env::var(HOST_ENV).unwrap_or(DEFAULT_CONTROL_HOST.to_string());

        let port = env::var(PORT_ENV).unwrap_or(DEFAULT_CONTROL_PORT.to_string());

        let scheme = if tls_off { "ws" } else { "wss" };
        let control_url = format!("{}://{}:{}/wormhole", scheme, control_host, port);

        info!("Control Server URL: {}", &control_url);

        Ok(Config {
            client_id: ClientId::generate(),
            local_host: opts.local_host,
            use_tls: opts.use_tls,
            control_url,
            host,
            local_port: opts.port,
            local_addr,
            sub_domain,
            dashboard_port: opts.dashboard_port.unwrap_or(0),
            verbose: opts.verbose,
            secret_key: secret_key.map(SecretKey),
            control_tls_off: tls_off,
            first_run: true,
        })
    }

    pub fn activation_url(&self, full_hostname: &str) -> String {
        format!(
            "{}://{}",
            if self.control_tls_off {
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
