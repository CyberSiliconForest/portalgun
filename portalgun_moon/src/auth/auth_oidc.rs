// SPDX-FileCopyrightText: 2023 perillamint <perillamint@silicon.moe>
//
// SPDX-License-Identifier: MIT

use anyhow::anyhow;
use async_trait::async_trait;
use jsonwebtoken::jwk::{AlgorithmParameters, Jwk, JwkSet};
use jsonwebtoken::DecodingKey;
use regex::Regex;
use serde::Deserialize;
use url::Url;

use super::{AuthResult, AuthService};

#[derive(Debug, Clone, Deserialize)]
struct OIDCJwksDiscovery {
    pub issuer: String,
    pub jwks_uri: Url,
}

#[derive(Debug, Clone, Deserialize)]
struct TokenPayload {
    // Standard Claims
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub exp: i64,
    pub iat: i64,

    pub claims: Option<TunnelClaims>,
    pub portalgun_subdomains: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
struct TunnelClaims {
    pub portalgun_subdomains: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct AuthOidcService {
    oidc_discovery_url: String,
    client_id: String,
    scopes: Vec<String>,
    jwks: Option<JwkSet>,
    issuer: Option<String>,
}

impl AuthOidcService {
    pub fn new(oidc_discovery_url: &str, client_id: &str, scopes: &str) -> Self {
        Self {
            oidc_discovery_url: oidc_discovery_url.to_owned(),
            client_id: client_id.to_owned(),
            jwks: None,
            issuer: None,
            scopes: scopes.split(',').map(|str| str.to_owned()).collect(),
        }
    }

    pub async fn init(&mut self) -> Result<(), anyhow::Error> {
        let discovery: OIDCJwksDiscovery =
            reqwest::get(&self.oidc_discovery_url).await?.json().await?;
        let jwks: JwkSet = reqwest::get(discovery.jwks_uri).await?.json().await?;

        self.jwks = Some(jwks);
        self.issuer = Some(discovery.issuer);
        Ok(())
    }

    pub fn get_configuration(&self) -> (String, String, Vec<String>) {
        (
            self.oidc_discovery_url.clone(),
            self.client_id.clone(),
            self.scopes.clone(),
        )
    }
}

#[async_trait]
impl AuthService for AuthOidcService {
    type Error = anyhow::Error;
    type AuthKey = String;

    /// Authenticate a subdomain with an AuthKey
    async fn auth_sub_domain(
        &self,
        auth_key: &Self::AuthKey,
        _subdomain: &str,
    ) -> Result<AuthResult, Self::Error> {
        let token_header = jsonwebtoken::decode_header(auth_key)?;

        let jwk: &Jwk = self
            .jwks
            .as_ref()
            .ok_or_else(|| anyhow!("Not initialized!"))?
            .find(&token_header.kid.ok_or_else(|| anyhow!("No KID!"))?)
            .ok_or_else(|| anyhow!("Key not found!"))?;

        let issuer = self.issuer.as_ref().ok_or_else(|| anyhow!("No issuer!"))?;

        let key = match jwk.algorithm {
            AlgorithmParameters::RSA(ref rsa) => DecodingKey::from_rsa_components(&rsa.n, &rsa.e),
            _ => return Err(anyhow!("Unsupported algorithm",)),
        }?;

        let validation = jsonwebtoken::Validation::new(token_header.alg);
        let decoded_token = jsonwebtoken::decode::<TokenPayload>(auth_key, &key, &validation)?;

        // Check issuer.
        if decoded_token.claims.iss != *issuer {
            return Err(anyhow!("Invalid issuer!"));
        }

        if decoded_token.claims.aud != self.client_id {
            return Err(anyhow!("Invalid audience!"));
        }

        if decoded_token.claims.exp < chrono::Utc::now().timestamp() {
            return Err(anyhow!("Expired token!"));
        }

        if decoded_token.claims.iat > chrono::Utc::now().timestamp() {
            return Err(anyhow!("Token issued in the future!"));
        }

        let subdomains: Vec<Result<Regex, _>> = match decoded_token.claims.portalgun_subdomains {
            Some(subdomains) => Some(subdomains),
            None => match decoded_token.claims.claims {
                Some(claims) => claims.portalgun_subdomains,
                None => None,
            },
        }
        .ok_or_else(|| anyhow!("No subdomains!"))?
        .into_iter()
        .map(|s| Regex::new(&s))
        .collect();

        let subdomain_regexps: Result<Vec<Regex>, _> = subdomains.into_iter().collect();

        for subdom in subdomain_regexps? {
            if subdom.is_match(_subdomain) {
                return Ok(AuthResult::Available);
            }
        }

        Err(anyhow!("No matching subdomains!"))
    }
}
