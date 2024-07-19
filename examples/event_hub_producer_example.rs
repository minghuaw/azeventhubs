use azeventhubs::producer::{ProducerClient, ProducerClientOptions, SendEventOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let _ = dotenv::from_filename(".env");

    let connection_string = std::env::var("EVENT_HUBS_CONNECTION_STRING")?;
    let event_hub_name = std::env::var("EVENT_HUB_NAME")?;
    let options = ProducerClientOptions::default();
    let mut producer_client =
        ProducerClient::new_from_connection_string(connection_string, event_hub_name, options)
            .await?;

    let partition_ids = producer_client.get_partition_ids().await?;

    for i in 0..300 {
        log::info!("Sending event {}", i);
        let event = format!("Hello, world {}!", i);
        let options = SendEventOptions::new().with_partition_id(&partition_ids[0]);
        producer_client.send_event(event, options).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(5000)).await;
    }

    producer_client.close().await?;

    Ok(())
}
