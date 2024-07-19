use azeventhubs::producer::{ProducerClient, ProducerClientOptions, SendEventOptions, TryAddError};

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

    let mut batch = producer_client.create_batch(Default::default()).await?;
    let options = SendEventOptions::new().with_partition_id(&partition_ids[0]);
    for i in 0..300 {
        let event = format!("Batch event {} in", i);
        if let Err(err) = batch.try_add(event) {
            match err {
                TryAddError::BatchFull(_) => {
                    producer_client.send_batch(batch, options.clone()).await?;
                    batch = producer_client.create_batch(Default::default()).await?;
                    log::info!("Batch sent");
                }
                TryAddError::Codec { source, .. } => {
                    log::error!("Error: {:?}", source);
                    break;
                }
            }
        }
    }
    producer_client.send_batch(batch, options).await?;

    producer_client.close().await?;

    Ok(())
}
