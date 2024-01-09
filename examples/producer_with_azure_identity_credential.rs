use azeventhubs::producer::{
    EventHubProducerClient, EventHubProducerClientOptions, SendEventOptions,
};
use azure_identity::DefaultAzureCredential;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let _ = dotenv::from_filename(".env");
    let namespace = std::env::var("EVENT_HUBS_NAMESPACE")?;
    let fqn = format!("{}.servicebus.windows.net", namespace);
    let event_hub_name = std::env::var("EVENT_HUB_NAME")?;
    let options = EventHubProducerClientOptions::default();
    let default_credential = DefaultAzureCredential::default();

    let mut producer_client = EventHubProducerClient::new_from_credential(
        fqn,
        event_hub_name,
        default_credential,
        options,
    )
    .await?;

    log::info!("Sending a test event");

    let event = "test connect using azure identity";
    let options = SendEventOptions::new().with_partition_id("0");
    producer_client.send_event(event, options).await?;

    log::info!("Done sending a test event");

    producer_client.close().await?;

    Ok(())
}
