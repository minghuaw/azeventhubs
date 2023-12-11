//! Types related to auth.
//!

// FIXME: Many are exact copies from the Service Bus crate. This should probably moved
// to a common crate.

pub use azure_named_key_credential::AzureNamedKeyCredential;
pub use azure_sas_credential::AzureSasCredential;
pub use event_hub_token_credential::EventHubTokenCredential;
pub use shared_access_credential::SharedAccessCredential;
pub use signautre_authorization_resource::*;

mod azure_named_key_credential;
mod azure_sas_credential;
mod signautre_authorization_resource;
pub(crate) mod event_hub_claim;
pub(crate) mod event_hub_token_credential;
pub(crate) mod shared_access_credential;
pub(crate) mod shared_access_signature;

cfg_not_wasm32! {
    #[cfg(test)]
    pub(crate) mod tests {
        use azure_core::auth::AccessToken;
        use azure_core::error::Result;

        use std::pin::Pin;
        use std::future::Future;

        use mockall::mock;

        mock! {
            #[derive(Debug)]
            pub TokenCredential {}

            impl azure_core::auth::TokenCredential for TokenCredential {
                // Required methods
                fn get_token<'life0, 'life1, 'life2, 'async_trait>(
                    &'life0 self,
                    scopes: &'life1 [&'life2 str]
                ) -> Pin<Box<dyn Future<Output = Result<AccessToken>> + Send + 'async_trait>>
                where Self: 'async_trait,
                        'life0: 'async_trait,
                        'life1: 'async_trait,
                        'life2: 'async_trait;

                fn clear_cache<'life0, 'async_trait>(
                    &'life0 self
                ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'async_trait>>
                where Self: 'async_trait,
                        'life0: 'async_trait;
            }
        }
    }
}
