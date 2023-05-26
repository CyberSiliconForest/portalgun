// SPDX-FileCopyrightText: 2023 perillamint <perillamint@silicon.moe>
//
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use url::Url;

use oauth2::basic::BasicClient;
use oauth2::devicecode::StandardDeviceAuthorizationResponse;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, ClientId, DeviceAuthorizationUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};

// Minimum OIDC Discovery parse struct
#[derive(Serialize, Deserialize, Debug)]
pub struct OIDCDiscovery {
    pub authorization_endpoint: Url,
    pub token_endpoint: Option<Url>,
    pub device_authorization_endpoint: Option<Url>, // Not included in OIDC standard, but used by some OAuth2 providers
}

pub async fn authorize(
    discovery_url: &str,
    client_id: &str,
    scopes: Vec<String>,
) -> Result<String, crate::Error> {
    let discovery: OIDCDiscovery = reqwest::get(discovery_url).await?.json().await?;

    // Create an OAuth2 client by specifying the client ID, client secret, authorization URL and
    // token URL.
    let client = BasicClient::new(
        ClientId::new(client_id.to_string()),
        None,
        AuthUrl::from_url(discovery.authorization_endpoint),
        Some(TokenUrl::from_url(discovery.token_endpoint.ok_or_else(
            || crate::Error::OAuth2("Token endpoint not found".to_owned()),
        )?)),
    )
    .set_device_authorization_url(DeviceAuthorizationUrl::from_url(
        discovery.device_authorization_endpoint.ok_or_else(|| {
            crate::Error::OAuth2("Device authorization endpoint not found".to_owned())
        })?,
    ));

    let mut auth_request = client
        .exchange_device_code()
        .map_err(|e| crate::Error::OAuth2(format!("Error: {}", e).to_owned()))?;

    for scope in scopes {
        auth_request = auth_request.add_scope(Scope::new(scope));
    }

    let details: StandardDeviceAuthorizationResponse = auth_request
        .request_async(async_http_client)
        .await
        .map_err(|e| crate::Error::OAuth2(format!("Error: {}", e).to_owned()))?;

    match details.verification_uri_complete() {
        Some(uri) => {
            eprintln!("Open this URL in your browser:\n{}", uri.secret());
        }
        None => {
            eprintln!(
                "Open this URL in your browser:\n{}\nand enter the code: {}",
                details.verification_uri().to_string(),
                details.user_code().secret().to_string()
            );
        }
    }

    let token_result = client
        .exchange_device_access_token(&details)
        .request_async(async_http_client, tokio::time::sleep, None)
        .await
        .map_err(|e| crate::Error::OAuth2(format!("Error: {}", e).to_owned()))?;

    Ok(token_result
        .refresh_token()
        .ok_or_else(|| crate::Error::OAuth2("Refresh token not found".to_owned()))?
        .secret()
        .clone())
}

pub async fn fetch_token(
    discovery_url: &str,
    client_id: &str,
    refresh_token: &str,
) -> Result<(String, Option<String>), crate::Error> {
    let discovery: OIDCDiscovery = reqwest::get(discovery_url).await?.json().await?;

    // Create an OAuth2 client by specifying the client ID, client secret, authorization URL and
    // token URL.
    let client = BasicClient::new(
        ClientId::new(client_id.to_string()),
        None,
        AuthUrl::from_url(discovery.authorization_endpoint),
        Some(TokenUrl::from_url(discovery.token_endpoint.ok_or_else(
            || crate::Error::OAuth2("Token endpoint not found".to_owned()),
        )?)),
    )
    .set_device_authorization_url(DeviceAuthorizationUrl::from_url(
        discovery.device_authorization_endpoint.ok_or_else(|| {
            crate::Error::OAuth2("Device authorization endpoint not found".to_owned())
        })?,
    ));

    let token_result = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token.to_owned()))
        .request_async(async_http_client)
        .await
        .map_err(|e| crate::Error::OAuth2(format!("Error: {}", e).to_owned()))?;

    Ok((
        token_result.access_token().secret().clone(),
        token_result.refresh_token().map(|t| t.secret().to_owned()),
    ))
}
