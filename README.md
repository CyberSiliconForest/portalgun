<!--
SPDX-FileCopyrightText: 2023 perillamint <perillamint@silicon.moe>
SPDX-FileCopyrightText: 2020-2022 Alex Grinman <me@alexgr.in>

SPDX-License-Identifier: MIT
-->

# `portalgun`
`portalgun` lets you expose your locally running web server via a public URL.
Written in Rust. Built completely with async-io on top of tokio.

1. [Install](#install)
2. [Usage Instructions](#usage)
3. [Host it yourself](#host-it-yourself)

# Install
## Cargo
```bash
cargo install portalgun
```

## Everywhere
Or **Download a release for your target OS here**: [portalgun/releases](https://github.com/perillamint/portalgun/releases)

# Usage
## Quick Start
```shell script
portalgun --port 8000
```
The above command opens a tunnel and forwards traffic to `localhost:8000`.

## More Options:
```shell script
Expose your local web server to the internet with a public url.

Usage: portalgun [OPTIONS] [COMMAND]

Commands:
  login  Login using OpenID Connect. This will store the authentication token on disk for future use
  help   Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose
          A level of verbosity, and can be used multiple times
  -s, --sub-domain <SUB_DOMAIN>
          Specify a sub-domain for this tunnel
      --host <LOCAL_HOST>
          Sets the HOST (i.e. localhost) to forward incoming tunnel traffic to [default: localhost]
  -t, --use-tls
          Sets the protocol for local forwarding (i.e. https://localhost) to forward incoming tunnel traffic to
      --port <PORT>
          Sets the port to forward incoming tunnel traffic to on the target host [default: 8000]
      --dashboard-port <DASHBOARD_PORT>
          Sets the address of the local introspection dashboard
  -h, --help
          Print help
```

# Host it yourself
1. See `Dockerfile` for a simple alpine based image that runs that server binary.
2. Deploy the image where ever you want.

## OIDC provider setup
Portalgun uses following custom attribute to determine user is allowed to create portal on specific subdomain.

```json
{
  "portalgun": [
    ".*",
    "or-any-subdomain-matching-regexp-in-here
  ]
}
```

Admins must configure their identity provider appropriately.

## Testing Locally
```shell script
# Run the Server: xpects TCP traffic on 8080 and control websockets on 5000
ALLOWED_HOSTS='localhost' TUNNEL_HOST='localhost' OIDC_DISCOVERY='https://example.com/.well-known/openid-configuration' OIDC_CLIENT_ID='openid-client-id-here' OIDC_SCOPES='openid,portalgun' cargo run --bin portalgun_moon

# Logging in using OIDC
cargo run --bin portalgun login --control-server ws://localhost:5000  

# Test it out!
# Remember 8080 is our local portalgun TCP server
curl -H '<subdomain>.localhost' "http://localhost:8080/some_path?with=somequery"
```
See `portalgun_moon/src/config.rs` for the environment variables for configuration.

