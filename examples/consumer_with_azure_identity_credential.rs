use azeventhubs::consumer::{
    EventHubConsumerClient, EventHubConsumerClientOptions, EventPosition, ReadEventOptions,
};
use azure_identity::DefaultAzureCredential;
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let _ = dotenv::from_filename(".env");

    let namespace = std::env::var("EVENT_HUBS_NAMESPACE")?;
    let fqn = format!("{}.servicebus.windows.net", namespace);
    let event_hub_name = std::env::var("EVENT_HUB_NAME")?;
    let options = EventHubConsumerClientOptions::default();
    let default_credential = DefaultAzureCredential::default();

    // Create a consumer client
    // let mut consumer_client = EventHubConsumerClient::new_from_connection_string(
    //     EventHubConsumerClient::DEFAULT_CONSUMER_GROUP_NAME,
    //     connection_string,
    //     event_hub_name,
    //     options,
    // )
    // .await?;

    let mut consumer_client = EventHubConsumerClient::new_from_credential(
        EventHubConsumerClient::DEFAULT_CONSUMER_GROUP_NAME,
        fqn,
        event_hub_name,
        default_credential,
        options,
    )
    .await?;

    let partition_ids = consumer_client.get_partition_ids().await?;
    let starting_position = EventPosition::earliest();
    let options = ReadEventOptions::default();

    // Get a stream of events from the first partition
    let mut stream = consumer_client
        .read_events_from_partition(&partition_ids[0], starting_position, options)
        .await?;

    // Receive 30 events
    let mut counter = 0;
    while let Some(event) = stream.next().await {
        let event = event?;
        let body = event.body()?;
        let value = std::str::from_utf8(body)?;
        log::info!("{:?}", value);

        log::info!("counter: {}", counter);
        counter += 1;
        if counter > 3000 {
            break;
        }
    }
    // Close the stream
    stream.close().await?;

    // Close the consumer client
    consumer_client.close().await?;

    Ok(())
}
