use async_trait::async_trait;

use super::{AuthResult, AuthService};

#[derive(Debug, Clone)]
pub struct AuthTokenService {
    token: String,
}

impl AuthTokenService {
    pub fn new(token: &str) -> Result<Self, ()> {
        Ok(Self {
            token: token.to_owned(),
        })
    }
}

#[async_trait]
impl AuthService for AuthTokenService {
    type Error = ();
    type AuthKey = String;

    /// Authenticate a subdomain with an AuthKey
    async fn auth_sub_domain(
        &self,
        auth_key: &Self::AuthKey,
        _subdomain: &str,
    ) -> Result<AuthResult, Self::Error> {
        if auth_key == &self.token {
            Ok(AuthResult::Available)
        } else {
            Err(())
        }
    }
}
