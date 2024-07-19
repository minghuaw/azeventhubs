use azure_core::auth::AccessToken;
use std::sync::Arc;

use crate::{
    authorization::event_hub_token_credential::EventHubTokenCredential,
    constants::{JSON_WEB_TOKEN_TYPE, SAS_TOKEN_TYPE},
};

#[derive(Debug)]
pub(crate) enum TokenType {
    /// The type to consider a token if it is based on an Event Hubs shared access signature.
    SharedAccessToken {
        credential: Arc<EventHubTokenCredential>,
    },
    /// The type to consider a token if not based on a shared access signature.
    JsonWebToken {
        credential: Arc<EventHubTokenCredential>,

        /// The JWT-based token that is currently cached for authorization.
        cached_token: Option<AccessToken>,
    },
}

impl TokenType {
    pub(crate) fn entity_type(&self) -> &str {
        match self {
            TokenType::SharedAccessToken { .. } => SAS_TOKEN_TYPE,
            TokenType::JsonWebToken { .. } => JSON_WEB_TOKEN_TYPE,
        }
    }
}

impl std::fmt::Display for TokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenType::SharedAccessToken { .. } => write!(f, "{}", SAS_TOKEN_TYPE),
            TokenType::JsonWebToken { .. } => write!(f, "{}", JSON_WEB_TOKEN_TYPE),
        }
    }
}