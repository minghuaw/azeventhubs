use azeventhubs::{consumer::{
    EventHubConsumerClient, EventHubConsumerClientOptions, EventPosition, ReadEventOptions,
}, BasicRetryPolicy};
use futures_util::StreamExt;

async fn read_events_from_partition(
    mut client: EventHubConsumerClient<BasicRetryPolicy>,
    partition_id: String,
    stop_after: usize,
) -> Result<(), azure_core::Error> {
    let starting_position = EventPosition::earliest();
    let options = ReadEventOptions::default();

    // Get a stream of events from the first partition
    log::info!("Reading from partition: {}", partition_id);
    let mut stream = client
        .read_events_from_partition(&partition_id, starting_position, options)
        .await?;

    let mut counter = 0;
    while let Some(event) = stream.next().await {
        let event = event?;
        let body = event.body()?;
        let value = std::str::from_utf8(body)?;
        log::info!("Partition {:?}, {:?}", partition_id, value);

        log::info!("counter: {}", counter);
        counter += 1;
        if counter > stop_after {
            break;
        }
    }
    // Close the stream
    stream.close().await?;
    client.close().await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let _ = dotenv::from_filename(".env");

    let connection_string = std::env::var("IOTHUB_BUILTIN_CONNECTION_STRING")?;
    // let event_hub_name = std::env::var("EVENT_HUB_NAME")?;
    let options = EventHubConsumerClientOptions::default();

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

    // stream.close().await?;
    drop(stream);
    consumer_client.close().await?;

    // let partition_ids = consumer_client.get_partition_ids().await?;

    // // This client is no longer needed
    // consumer_client.close().await?;

    // tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // let mut handles = Vec::new();
    // for partition_id in partition_ids {
    //     let connection_string_clone = connection_string.clone();
    //     let options = EventHubConsumerClientOptions::default();
    //     let handle = tokio::spawn(async move {
    //         log::info!("Spawning partition: {}", partition_id);
    //         let client = EventHubConsumerClient::new_from_connection_string(
    //             EventHubConsumerClient::DEFAULT_CONSUMER_GROUP_NAME,
    //             connection_string_clone,
    //             None,
    //             options,
    //         )
    //         .await?;
    //         read_events_from_partition(client, partition_id, 3000).await
    //     });
    //     handles.push(handle);

    //     // Wait a bit before spawning the next partition so that we can more easily distinguish
    //     // them in the log
    //     tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    // }

    // let results = futures_util::future::join_all(handles).await;
    // for result in results {
    //     result??;
    // }

    Ok(())
}
