use std::{collections::VecDeque, time::Duration as StdDuration};

use fe2o3_amqp::{link::RecvError, session::SessionHandle, Receiver};
use futures_util::FutureExt;
use tokio::sync::mpsc;

use crate::{
    consumer::EventPosition,
    core::{RecoverableError, RecoverableTransport, TransportClient},
    event_hubs_retry_policy::EventHubsRetryPolicy,
    util::{self, time::timeout},
    ReceivedEventData,
};

use crate::amqp::{
    amqp_cbs_link::Command,
    amqp_client::AmqpClient,
    error::{DisposeConsumerError, RecoverAndReceiveError},
};

use super::{Consumer, ConsumerOptions, EventStream, EventStreamState};

#[derive(Debug)]
pub struct AmqpConsumer<RP> {
    pub(crate) session_handle: SessionHandle<()>,
    pub(crate) _session_identifier: u32,
    pub(crate) receiver: Receiver,
    pub(crate) link_identifier: u32,
    pub(crate) track_last_enqueued_event_properties: bool,
    pub(crate) last_received_event: Option<ReceivedEventData>,
    pub(crate) current_event_position: Option<EventPosition>,
    pub(crate) retry_policy: RP,
    pub(crate) prefetch_count: u32,
    pub(crate) cbs_command_sender: mpsc::Sender<Command>,

    pub(crate) initial_options: ConsumerOptions,
}

impl<RP> AmqpConsumer<RP> {
    pub async fn close(mut self) -> Result<(), DisposeConsumerError> {
        // There is no need to remove the refresher if CBS link is already stopped
        let _ = self
            .cbs_command_sender
            .send(Command::RemoveAuthorizationRefresher(self.link_identifier))
            .await;

        self.receiver.close().await?;
        self.session_handle.close().await?;
        drop(self.session_handle);
        Ok(())
    }

    pub(crate) async fn recv_and_accept(&mut self) -> Result<ReceivedEventData, RecvError> {
        if self.prefetch_count == 0 {
            // At least one credit is needed
            self.receiver.set_credit(1).await?;
        }

        let delivery = self.receiver.recv().await?;
        self.receiver.accept(&delivery).await?;
        let event = ReceivedEventData::from_raw_amqp_message(delivery.into_message());

        let event_offset = event.offset().unwrap_or(i64::MIN);
        if event_offset > i64::MIN {
            self.current_event_position = Some(EventPosition::from_offset(event_offset, false));
        }

        if self.track_last_enqueued_event_properties {
            self.last_received_event = Some(event.clone());
        }

        Ok(event)
    }

    #[inline]
    async fn fill_buf(
        &mut self,
        buffer: &mut VecDeque<ReceivedEventData>,
    ) -> Result<(), RecvError> {
        // Only receive messages if there is space in the buffer
        let max_messages = buffer.capacity() - buffer.len();
        // Credit mode is manual, need to set credit
        if self.prefetch_count == 0 {
            // At least one credit is needed
            // max_messages is specified as u32, so it is safe to cast to u32
            let credit = max_messages.max(1) as u32;
            self.receiver.set_credit(credit).await?;
        }

        for _ in 0..max_messages {
            let delivery = self.receiver.recv().await?;
            self.receiver.accept(&delivery).await?;
            let event = ReceivedEventData::from_raw_amqp_message(delivery.into_message());

            let event_offset = event.offset().unwrap_or(i64::MIN);
            if event_offset > i64::MIN {
                self.current_event_position = Some(EventPosition::from_offset(event_offset, false));
            }

            buffer.push_back(event);
        }

        Ok(())
    }

    #[inline]
    async fn fill_buf_with_timeout(
        &mut self,
        buffer: &mut VecDeque<ReceivedEventData>,
        max_wait_time: StdDuration,
    ) -> Result<(), RecoverAndReceiveError> {
        futures_util::select_biased! {
            _ = crate::util::time::sleep(max_wait_time).fuse() => Ok(()),
            result = self.fill_buf(buffer).fuse() => {
                result?;
                Ok(())
            }
        }
    }
}

async fn recover_and_recv_batch<RP>(
    client: &mut AmqpClient,
    consumer: &mut AmqpConsumer<RP>,
    should_try_recover: bool,
    buffer: &mut VecDeque<ReceivedEventData>,
    max_wait_time: StdDuration,
) -> Result<(), RecoverAndReceiveError>
where
    RP: EventHubsRetryPolicy + Send,
{
    if should_try_recover {
        if let Err(recovery_err) = client.recover().await {
            log::error!("Failed to recover client: {:?}", recovery_err);
            if recovery_err.is_scope_disposed() {
                return Err(recovery_err.into());
            }
        }

        recover_consumer_by_creating_new_consumer(client, consumer).await?;
    }

    consumer
        .fill_buf_with_timeout(buffer, max_wait_time)
        .await?;
    if consumer.track_last_enqueued_event_properties {
        if let Some(event) = buffer.back().cloned() {
            consumer.last_received_event = Some(event);
        }
    }
    Ok(())
}

async fn recover_and_recv<RP>(
    client: &mut AmqpClient,
    consumer: &mut AmqpConsumer<RP>,
    should_try_recover: bool,
) -> Result<ReceivedEventData, RecoverAndReceiveError>
where
    RP: EventHubsRetryPolicy + Send,
{
    if should_try_recover {
        if let Err(recovery_err) = client.recover().await {
            log::error!("Failed to recover client: {:?}", recovery_err);
            if recovery_err.is_scope_disposed() {
                return Err(recovery_err.into());
            }
        }

        recover_consumer_by_creating_new_consumer(client, consumer).await?;
    }

    consumer.recv_and_accept().await.map_err(Into::into)
}

pub(crate) async fn recover_consumer_by_creating_new_consumer(
    client: &mut AmqpClient,
    consumer: &mut AmqpConsumer<impl EventHubsRetryPolicy + Send>,
) -> Result<(), RecoverAndReceiveError> {
    log::debug!("Recovering consumer by creating new consumer");

    let consumer_group = &consumer.initial_options.consumer_group;
    let partition_id = &consumer.initial_options.partition_id;
    let mut event_position = consumer
        .current_event_position
        .clone()
        .unwrap_or(consumer.initial_options.event_position.clone());
    match &mut event_position {
        EventPosition::Offset { is_inclusive, .. } => *is_inclusive = false,
        EventPosition::SequenceNumber { is_inclusive, .. } => *is_inclusive = false,
        EventPosition::EnqueuedTime(_) => {}
    }
    let prefetch_count = consumer.initial_options.prefetch_count;
    let owner_level = consumer.initial_options.owner_level;
    let track_last_enqueued_event_properties =
        consumer.initial_options.track_last_enqueued_event_properties;
    let identifier = consumer.initial_options.identifier.clone();
    let retry_policy = consumer.retry_policy.clone();

    let new_consumer = client
        .create_consumer(
            consumer_group,
            partition_id,
            identifier,
            &event_position,
            retry_policy,
            track_last_enqueued_event_properties,
            owner_level,
            Some(prefetch_count),
        )
        .await?;

    let old_consumer = std::mem::replace(consumer, new_consumer);
    let _ = old_consumer.close().await;

    Ok(())
}

pub(crate) async fn receive_event_batch<RP>(
    client: &mut AmqpClient,
    consumer: &mut AmqpConsumer<RP>,
    buffer: &mut VecDeque<ReceivedEventData>,
    max_wait_time: Option<StdDuration>,
) -> Result<(), RecoverAndReceiveError>
where
    RP: EventHubsRetryPolicy + Send,
{
    let mut failed_attempts = 0;
    let mut try_timeout = consumer.retry_policy.calculate_try_timeout(failed_attempts);
    let mut should_try_recover = false;

    loop {
        let wait_time = max_wait_time.unwrap_or(try_timeout);
        let err =
            match recover_and_recv_batch(client, consumer, should_try_recover, buffer, wait_time)
                .await
            {
                Ok(_) => return Ok(()),
                Err(err) => err,
            };

        log::debug!("Failed to receive batch: {:?}", err);

        if err.is_scope_disposed() {
            return Err(err);
        }
        should_try_recover = err.should_try_recover();

        failed_attempts += 1;
        let retry_delay = consumer
            .retry_policy
            .calculate_retry_delay(&err, failed_attempts);

        match retry_delay {
            Some(retry_delay) => {
                util::time::sleep(retry_delay).await;
                try_timeout = consumer.retry_policy.calculate_try_timeout(failed_attempts);
            }
            None => return Err(err),
        }
    }
}

pub(crate) async fn receive_event<RP>(
    client: &mut AmqpClient,
    consumer: &mut AmqpConsumer<RP>,
) -> Result<ReceivedEventData, RecoverAndReceiveError>
where
    RP: EventHubsRetryPolicy + Send,
{
    let mut failed_attempts = 0;
    let mut try_timeout = consumer.retry_policy.calculate_try_timeout(failed_attempts);
    let mut should_try_recover = false;

    loop {
        let err = match timeout(
            try_timeout,
            recover_and_recv(client, consumer, should_try_recover),
        )
        .await
        {
            Ok(result) => match result {
                Ok(event) => return Ok(event),
                Err(err) => err,
            },
            Err(_try_timeout_elapsed) => {
                // There is no error returned from client, everything is fine and keep waiting
                // TODO: is this correct?
                let credit = u32::max(1, consumer.prefetch_count);
                match consumer.receiver.set_credit(credit).await {
                    Ok(_) => continue,
                    Err(err) => RecoverAndReceiveError::from(err),
                }
            }
        };

        log::debug!("Failed to receive event: {:?}", err);

        if err.is_scope_disposed() {
            return Err(err);
        }
        should_try_recover = err.should_try_recover();

        failed_attempts += 1;
        let retry_delay = consumer
            .retry_policy
            .calculate_retry_delay(&err, failed_attempts);

        match retry_delay {
            Some(retry_delay) => {
                util::time::sleep(retry_delay).await;
                try_timeout = consumer.retry_policy.calculate_try_timeout(failed_attempts);
            }
            None => return Err(err),
        }
    }
}

pub(crate) struct EventStreamStateValue<'a, C> {
    pub(crate) client: &'a mut AmqpClient,
    pub(crate) consumer: C,
}

impl<'a, C> EventStreamStateValue<'a, C> {
    pub(crate) fn new(client: &'a mut AmqpClient, consumer: C) -> Self {
        Self { client, consumer }
    }
}

impl<'a, RP> EventStream<'a, RP>
where
    RP: Send + 'a,
    AmqpConsumer<RP>: Send + 'a,
{
    pub(crate) fn with_consumer(client: &'a mut AmqpClient, consumer: AmqpConsumer<RP>) -> Self {
        let consumer = Consumer::Single(consumer);
        let value = EventStreamStateValue::new(client, consumer);
        let state = EventStreamState::Value { value };

        Self { state }
    }
}
