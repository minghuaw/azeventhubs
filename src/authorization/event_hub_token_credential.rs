use azure_core::auth::{TokenCredential, AccessToken};

use super::shared_access_credential::SharedAccessCredential;

// FIXME: This is an exact copy from the Service Bus crate. This should probably moved
// to a common crate.
/// Provides a generic token-based credential for a given Event Hub instance.
///
/// This supports [`SharedAccessCredential`] and any other credential type that implements
/// [`TokenCredential`], eg. [`azure_identity::DefaultAzureCredential`].
///
/// # Example
///
/// ```rust, no_run
/// use azure_identity::{DefaultAzureCredential, TokenCredentialOptions};
/// use azeventhubs::authorization::EventHubTokenCredential;
///
/// let default_credential = DefaultAzureCredential::create(TokenCredentialOptions::default()).unwrap();
/// let credential = EventHubTokenCredential::from(default_credential);
/// ```
pub enum EventHubTokenCredential {
    // FIXME: This is a temporary workaround until specialization is stablized.
    /// Shared Access Signature credential.
    SharedAccessCredential(SharedAccessCredential),

    // TODO: Is the use of trait object here justified?
    /// Other credential types.
    Other(Box<dyn TokenCredential>),
}

impl std::fmt::Debug for EventHubTokenCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SharedAccessCredential(_arg0) => f.debug_tuple("SharedAccessCredential").finish(),
            Self::Other(_arg0) => f.debug_tuple("Other").finish(),
        }
    }
}

impl From<SharedAccessCredential> for EventHubTokenCredential {
    fn from(source: SharedAccessCredential) -> Self {
        Self::SharedAccessCredential(source)
    }
}

impl<TC> From<TC> for EventHubTokenCredential
where
    TC: TokenCredential + 'static,
{
    fn from(source: TC) -> Self {
        Self::Other(Box::new(source) as Box<dyn TokenCredential>)
    }
}

impl EventHubTokenCredential {
    /// Creates a new instance of [`EventHubTokenCredential`]. This is simply an alias for
    /// [`From::from`]
    pub fn new(source: impl Into<Self>) -> Self {
        source.into()
    }

    /// Indicates whether the credential is based on an Event Hubs
    /// shared access policy.
    pub fn is_shared_access_credential(&self) -> bool {
        matches!(self, EventHubTokenCredential::SharedAccessCredential(_))
    }
}

impl EventHubTokenCredential {
    pub(crate) const DEFAULT_SCOPE: &'static str = "https://eventhubs.azure.net/.default";

    /// Gets a `AccessToken` for the specified resource
    pub(crate) async fn get_token(&self, scopes: &[&str]) -> azure_core::Result<AccessToken> {
        match self {
            EventHubTokenCredential::SharedAccessCredential(credential) => {
                credential.get_token(scopes).await
            }
            EventHubTokenCredential::Other(credential) => credential.get_token(scopes).await,
        }
    }

    pub(crate) async fn get_token_using_default_resource(&self) -> azure_core::Result<AccessToken> {
        self.get_token(&[Self::DEFAULT_SCOPE]).await
    }
}

cfg_not_wasm32! {
    #[cfg(test)]
    mod tests {
        use azure_core::auth::Secret;
        use time::macros::datetime;

        use crate::authorization::{
            shared_access_credential::SharedAccessCredential,
            shared_access_signature::SharedAccessSignature,
        };

        use super::EventHubTokenCredential;

        #[tokio::test]
        async fn get_token_delegates_to_the_source_credential() {
            let token_value = "token";
            let mut mock_credentials = crate::authorization::tests::MockTokenCredential::new();
            let resource = "the resource value";
            let token_response = azure_core::auth::AccessToken {
                token: Secret::new(token_value),
                expires_on: datetime!(2015-10-27 00:00:00).assume_utc(),
            };
            mock_credentials
                .expect_get_token()
                .times(1)
                .returning(move |_resource| {
                    let token_response_clone = token_response.clone();
                    Box::pin( async { Ok(token_response_clone) } )
                });

            let credential = EventHubTokenCredential::from(mock_credentials);
            let token_result = credential.get_token(&[resource]).await;
            assert_eq!(token_result.unwrap().token.secret(), token_value);
        }

        #[test]
        fn is_shared_access_credential_recognized_as_sas_credentials() {
            let signature = SharedAccessSignature::try_from_parts(
                "sb-name",
                "keyName",
                "key",
                Some(std::time::Duration::from_secs(4 * 60 * 60)),
            )
            .unwrap();
            let sas_credential = SharedAccessCredential::from(signature);
            let credential = EventHubTokenCredential::new(sas_credential);
            assert!(credential.is_shared_access_credential());
        }

        #[tokio::test]
        async fn create_credential_with_azure_identity() {
            use azure_identity::{DefaultAzureCredential, TokenCredentialOptions};

            let default_credential = DefaultAzureCredential::create(TokenCredentialOptions::default()).unwrap();
            let event_hub_token_credential = EventHubTokenCredential::from(default_credential);
            let token = event_hub_token_credential
                .get_token_using_default_resource()
                .await
                .unwrap();
            assert!(!token.token.secret().is_empty())
        }
    }
}
