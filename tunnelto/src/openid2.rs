// SPDX-FileCopyrightText: 2023 perillamint <perillamint@silicon.moe>
//
// SPDX-License-Identifier: MIT

use url::Url;
use serde::Deserialize;

// Minimum OIDC Discovery parse struct
#[derive(Deserialize, Debug)]
struct OIDCDiscovery {
    pub issuer: Url,
    pub authorization_endpoint: Url,
    pub token_endpoint: Option<Url>,
    pub userinfo_endpoint: Option<Url>,
    pub end_session_endpoint: Option<Url>,
    pub introspection_endpoint: Option<Url>,
    pub revocation_endpoint: Option<Url>,
    pub device_authorization_endpoint: Option<Url>, // Not included in OIDC standard, but used by some OAuth2 providers

    pub jwks_uri: Url,
}
