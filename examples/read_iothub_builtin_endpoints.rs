use azeventhubs::consumer::{
    EventHubConsumerClient, EventHubConsumerClientOptions, ReadEventOptions,
};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let _ = dotenv::from_filename(".env");

    let connection_string = std::env::var("IOTHUB_BUILTIN_CONNECTION_STRING")?;
    let options = EventHubConsumerClientOptions::default();

    // Create a consumer client
    let mut consumer_client = EventHubConsumerClient::new_from_connection_string(
        // EventHubConsumerClient::DEFAULT_CONSUMER_GROUP_NAME,
        "$default",
        connection_string.clone(),
        None,
        options,
    )
    .await?;

    // // Idling for more than 1 minute will cause the connection to be closed. 
    // // This tests whether the client can recover from a closed connection.
    // tokio::time::sleep(std::time::Duration::from_secs(2 * 60)).await;

    let options = ReadEventOptions::default();
    let start_position = azeventhubs::consumer::EventPosition::latest();
    // let mut stream = consumer_client.read_events(false, options).await?;
    let mut stream = consumer_client.read_events_from_partition("0", start_position, options).await?;

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
