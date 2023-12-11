//! To run the example with logs printed to stdout, run the following command:
//!
//! ```bash
//! RUST_LOG=info cargo run --example spawn_multiple_consumer
//! ```

use std::time::Duration;

use azeventhubs::consumer::{
    EventHubConsumerClient, EventHubConsumerClientOptions, EventPosition, ReadEventOptions,
};
use futures_util::StreamExt;
use tokio_util::sync::CancellationToken;

async fn consumer_main(
    index: usize,
    connection_string: impl Into<String>,
    event_hub_name: impl Into<String>,
    client_options: EventHubConsumerClientOptions,
    cancel: CancellationToken,
) -> Result<(), azure_core::Error> {
    let mut consumer = EventHubConsumerClient::new_from_connection_string(
        EventHubConsumerClient::DEFAULT_CONSUMER_GROUP_NAME,
        connection_string,
        event_hub_name.into(),
        client_options,
    )
    .await?;
    let partition_ids = consumer.get_partition_ids().await?;
    let partition_id = &partition_ids[index];
    let starting_position = EventPosition::earliest();
    let read_event_options = ReadEventOptions::default();

    let mut stream = consumer
        .read_events_from_partition(partition_id, starting_position, read_event_options)
        .await?;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                log::info!("{}: Cancelled", partition_id);
                break;
            },
            event = stream.next() => {
                match event {
                    Some(Ok(event)) => {
                        let body = event.body()?;
                        let value = std::str::from_utf8(body)?;
                        log::info!("{}: {:?}", partition_id, value);
                    },
                    Some(Err(e)) => {
                        log::error!("{}: {:?}", partition_id, e);
                    },
                    None => {
                        log::info!("{}: Stream closed", partition_id);
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    dotenv::from_filename(".env")?;

    let connection_string = std::env::var("EVENT_HUBS_CONNECTION_STRING")?;
    let event_hub_name = std::env::var("EVENT_HUB_NAME")?;
    let client_options = EventHubConsumerClientOptions::default();

    // We are going to use a cancellation token to stop the spawned tasks.
    let cancel = CancellationToken::new();
    // Assuming that there are three partitions, and we will create one consumer for each partition.
    let mut handles = Vec::new();
    for i in 0..3 {
        let handle = tokio::spawn(consumer_main(
            i,
            connection_string.clone(),
            event_hub_name.clone(),
            client_options.clone(),
            cancel.child_token(),
        ));
        handles.push(handle);
    }

    // Wait for 10 seconds and then cancel the spawned tasks.
    tokio::time::sleep(Duration::from_secs(10)).await;
    cancel.cancel();
    for handle in handles {
        handle.await??;
    }

    Ok(())
}
