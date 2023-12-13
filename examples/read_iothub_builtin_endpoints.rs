use azeventhubs::{
    consumer::{
        EventHubConsumerClient, EventHubConsumerClientOptions, ReadEventOptions,
    },
    EventHubsRetryOptions, MaxRetries,
};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let _ = dotenv::from_filename(".env");

    let connection_string = std::env::var("IOTHUB_BUILTIN_CONNECTION_STRING")?;
    // let event_hub_name = std::env::var("EVENT_HUB_NAME")?;
    let retry_options = EventHubsRetryOptions {
        max_retries: MaxRetries::try_from(0).unwrap(),
        ..Default::default()
    };
    let options = EventHubConsumerClientOptions {
        retry_options,
        ..Default::default()
    };

    // Create a consumer client
    let mut consumer_client = EventHubConsumerClient::new_from_connection_string(
        EventHubConsumerClient::DEFAULT_CONSUMER_GROUP_NAME,
        connection_string.clone(),
        None,
        options,
    )
    .await?;
    let options = ReadEventOptions::default();

    log::info!("Sleeping for 2 minutes");
    tokio::time::sleep(std::time::Duration::from_secs(2 * 60)).await;

    log::info!("Reading from all partitions");
    let mut stream = consumer_client.read_events(false, options).await?;

    let mut counter = 0;
    while let Some(event) = stream.next().await {
        let event = event?;
        let body = event.body()?;
        let value = std::str::from_utf8(body)?;
        log::info!("{:?}", value);

        log::info!("counter: {}", counter);
        counter += 1;
        if counter > 100 {
            break;
        }
    }

    stream.close().await?;
    consumer_client.close().await?;

    Ok(())
}
