#![cfg(all(test, feature = "test_e2e"))]

use std::cfg;

use azeventhubs::{Connection, ConnectionOptions, TransportType};
use azure_identity::TokenCredentialOptions;

#[macro_use]
mod cfg;

mod common;

cfg_not_wasm32! {
    #[tokio::test]
    async fn connection_can_connect_to_event_hubs_using_full_connection_string_over_tcp() {
        common::setup_dotenv();

        let connection_string = std::env::var("EVENT_HUBS_CONNECTION_STRING_WITH_ENTITY_PATH").unwrap();
        let options = ConnectionOptions::default();
        let connection = Connection::new_from_connection_string(connection_string, None, options)
            .await
            .unwrap();
        connection.close().await.unwrap();
    }

    #[tokio::test]
    async fn connection_can_connect_to_event_hubs_using_full_connection_string_and_event_hub_over_websockets(
    ) {
        common::setup_dotenv();

        let connection_string = std::env::var("EVENT_HUBS_CONNECTION_STRING").unwrap();
        let event_hub_name = std::env::var("EVENT_HUB_NAME").unwrap();
        let mut options = ConnectionOptions::default();
        options.transport_type = TransportType::AmqpWebSockets;
        let connection =
            Connection::new_from_connection_string(connection_string, event_hub_name, options)
                .await
                .unwrap();
        connection.close().await.unwrap();
    }

    #[tokio::test]
    async fn connection_can_connect_to_event_hubs_using_full_connection_string_and_event_hub_over_tcp()
    {
        common::setup_dotenv();

        let connection_string = std::env::var("EVENT_HUBS_CONNECTION_STRING").unwrap();
        let event_hub_name = std::env::var("EVENT_HUB_NAME").unwrap();
        let options = ConnectionOptions::default();
        let connection =
            Connection::new_from_connection_string(connection_string, event_hub_name, options)
                .await
                .unwrap();
        connection.close().await.unwrap();
    }

    #[tokio::test]
    async fn connection_can_connect_with_named_key_credential() {
        common::setup_dotenv();
        use azeventhubs::authorization::AzureNamedKeyCredential;

        let options = ConnectionOptions::default();

        let namespace = std::env::var("EVENT_HUBS_NAMESPACE").unwrap();
        let fqn = format!("{}.servicebus.windows.net", namespace);
        let event_hub_name = std::env::var("EVENT_HUB_NAME").unwrap();
        let key_name = std::env::var("EVENT_HUBS_SHARED_ACCESS_KEY_NAME").unwrap();
        let key = std::env::var("EVENT_HUBS_SHARED_ACCESS_KEY").unwrap();

        let named_key_credential = AzureNamedKeyCredential::new(key_name, key);

        let connection = Connection::new_from_named_key_credential(
            fqn,
            event_hub_name,
            named_key_credential,
            options,
        ).await.unwrap();
        connection.close().await.unwrap();
    }

    #[tokio::test]
    async fn connection_can_connect_with_azure_identity_credential() {
        common::setup_dotenv();

        use azure_identity::DefaultAzureCredential;

        let namespace = std::env::var("EVENT_HUBS_NAMESPACE").unwrap();
        let fqn = format!("{}.servicebus.windows.net", namespace);
        let event_hub_name = std::env::var("EVENT_HUB_NAME").unwrap();

        let options = ConnectionOptions::default();
        let credential = DefaultAzureCredential::create(TokenCredentialOptions::default()).unwrap();

        let connection = Connection::new_from_credential(
            fqn,
            event_hub_name,
            credential,
            options,
        ).await.unwrap();

        connection.close().await.unwrap();
    }
}
