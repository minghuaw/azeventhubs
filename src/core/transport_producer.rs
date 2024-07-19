use crate::{
    producer::{CreateBatchOptions, SendEventOptions},
    EventData,
};

use super::transport_event_batch::TransportEventBatch;

pub(crate) trait TransportProducer {
    type MessageBatch: TransportEventBatch;

    type SendError: std::error::Error;
    type CreateBatchError: std::error::Error;

    fn create_batch(
        &self,
        options: CreateBatchOptions,
    ) -> Result<Self::MessageBatch, Self::CreateBatchError>;

    async fn send(
        &mut self,
        events: impl ExactSizeIterator<Item = EventData> + Send,
        options: SendEventOptions,
    ) -> Result<(), Self::SendError>;

    async fn send_batch(
        &mut self,
        batch: Self::MessageBatch,
        options: SendEventOptions,
    ) -> Result<(), Self::SendError>;
}
