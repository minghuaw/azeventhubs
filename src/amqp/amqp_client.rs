use std::sync::{atomic::Ordering, Arc};

use url::Url;

use crate::{
    amqp::amqp_management::event_hub_properties::EventHubPropertiesRequest,
    authorization::{event_hub_claim, event_hub_token_credential::EventHubTokenCredential},
    consumer::EventPosition,
    core::{RecoverableTransport, TransportClient, TransportProducerFeatures},
    event_hubs_connection_option::EventHubConnectionOptions,
    event_hubs_properties::EventHubProperties,
    event_hubs_retry_policy::EventHubsRetryPolicy,
    producer::PartitionPublishingOptions,
    util::sharable::Sharable,
    PartitionProperties,
};

use super::{
    amqp_connection_scope::AmqpConnectionScope,
    amqp_consumer::AmqpConsumer,
    amqp_management::partition_properties::PartitionPropertiesRequest,
    amqp_management_link::AmqpManagementLink,
    amqp_producer::AmqpProducer,
    error::{
        AmqpClientError, DisposeError, OpenConsumerError, OpenProducerError,
        RecoverConsumerError, RecoverProducerError, RecoverTransportClientError, RequestResponseError,
    },
};

const DEFAULT_PREFETCH_COUNT: u32 = 300;

#[derive(Debug)]
pub struct AmqpClient {
    pub(crate) connection_scope: AmqpConnectionScope,
    pub(crate) management_link: Sharable<AmqpManagementLink>,
}

impl AmqpClient {
    pub(crate) fn clone_as_shared(&mut self) -> Self {
        let shared_mgmt_link = self.management_link.clone_as_shared();
        let shared_mgmt_link = match shared_mgmt_link {
            Some(shared_mgmt_link) => Sharable::Shared(shared_mgmt_link),
            None => Sharable::None,
        };

        Self {
            connection_scope: self.connection_scope.clone_as_shared(),
            management_link: shared_mgmt_link,
        }
    }
}

impl AmqpClient {
    pub(crate) async fn new(
        host: &str,
        event_hub_name: Arc<String>,
        credential: EventHubTokenCredential,
        options: EventHubConnectionOptions,
    ) -> Result<Self, AmqpClientError> {
        // Scheme of service endpoint must always be either "amqp" or "amqps"
        let service_endpoint = format!("{}://{}", options.transport_type.url_scheme(), host);
        let service_endpoint = Url::parse(&service_endpoint)?;

        let connection_endpoint = match options.custom_endpoint_address {
            Some(mut url) => {
                url.set_scheme(options.transport_type.url_scheme())
                    .map_err(|_| AmqpClientError::SetUrlScheme)?;
                url
            }
            None => service_endpoint.clone(),
        };

        // Create AmqpConnectionScope
        let mut connection_scope = AmqpConnectionScope::new(
            service_endpoint,
            connection_endpoint,
            event_hub_name,
            credential,
            options.transport_type,
            options.connection_idle_timeout,
            None,
        )
        .await?;

        // Create AmqpManagementLink
        let management_link = connection_scope.open_management_link().await?;
        let management_link = Sharable::Owned(management_link);
        Ok(Self {
            connection_scope,
            management_link,
        })
    }
}

impl TransportClient for AmqpClient {
    type Producer<RP> = AmqpProducer<RP> where RP: EventHubsRetryPolicy + Send;
    type Consumer<RP> = AmqpConsumer<RP> where RP: EventHubsRetryPolicy + Send;

    type RequestResponseError = RequestResponseError;
    type OpenProducerError = OpenProducerError;
    type RecoverProducerError = RecoverProducerError;
    type OpenConsumerError = OpenConsumerError;
    type RecoverConsumerError = RecoverConsumerError;
    type DisposeError = DisposeError;

    fn is_closed(&self) -> bool {
        self.connection_scope.is_disposed.load(Ordering::Relaxed)
    }

    async fn get_properties(
        &mut self,
    ) -> Result<EventHubProperties, Self::RequestResponseError> {
        let access_token = self
            .connection_scope
            .credential
            .get_token_using_default_resource()
            .await?;
        let token_value = access_token.token.secret();

        let request =
            EventHubPropertiesRequest::new(&*self.connection_scope.event_hub_name, token_value);

        self.management_link.call(request).await
            .map_err(Into::into)
    }

    async fn get_partition_properties(
        &mut self,
        partition_id: &str,
    ) -> Result<PartitionProperties, Self::RequestResponseError> {
        let access_token = self
            .connection_scope
            .credential
            .get_token_using_default_resource()
            .await?;
        let token_value = access_token.token.secret();
        
        let request = PartitionPropertiesRequest::new(
            &*self.connection_scope.event_hub_name,
            partition_id,
            token_value,
        );

        self.management_link.call(request).await
            .map_err(Into::into)
    }

    async fn create_producer<RP>(
        &mut self,
        partition_id: Option<String>,
        producer_identifier: Option<String>,
        requested_features: TransportProducerFeatures,
        partition_options: PartitionPublishingOptions,
        retry_policy: RP,
    ) -> Result<Self::Producer<RP>, Self::OpenProducerError>
    where
        RP: EventHubsRetryPolicy + Send,
    {
        self.connection_scope.open_producer_link(
            partition_id,
            requested_features,
            partition_options,
            producer_identifier,
            retry_policy,
        ).await
    }

    async fn recover_producer<RP>(
        &mut self,
        producer: &mut Self::Producer<RP>,
    ) -> Result<(), Self::RecoverProducerError>
    where
        RP: EventHubsRetryPolicy + Send,
    {
        log::debug!("Recovering producer");

        let endpoint = producer.endpoint.to_string();
        let resource = endpoint.clone();
        let required_claims = vec![event_hub_claim::SEND.to_string()];
        self.connection_scope
            .request_refreshable_authorization_using_cbs(
                producer.link_identifier,
                endpoint,
                resource,
                required_claims,
            )
            .await?;

        if producer.session_handle.is_ended() {
            let new_session = self.connection_scope.connection.begin_session().await?;
            producer
                .sender
                .detach_then_resume_on_session(&new_session)
                .await?;
            producer.session_handle = new_session;
        } else {
            producer
                .sender
                .detach_then_resume_on_session(&producer.session_handle)
                .await?
        };

        log::debug!("Producer recovered");

        Ok(())
    }

    async fn create_consumer<RP>(
        &mut self,
        consumer_group: &str,
        partition_id: &str,
        consumer_identifier: Option<String>,
        event_position: &EventPosition,
        retry_policy: RP,
        track_last_enqueued_event_properties: bool,
        owner_level: Option<i64>,
        prefetch_count: Option<u32>,
    ) -> Result<Self::Consumer<RP>, Self::OpenConsumerError>
    where
        RP: EventHubsRetryPolicy + Send,
    {
        self.connection_scope.open_consumer_link(
            consumer_group,
            partition_id,
            event_position,
            prefetch_count.unwrap_or(DEFAULT_PREFETCH_COUNT),
            owner_level,
            track_last_enqueued_event_properties,
            consumer_identifier,
            retry_policy,
        ).await
    }

    // async fn recover_consumer<RP>(
    //     &mut self,
    //     consumer: &mut Self::Consumer<RP>,
    // ) -> Result<(), Self::RecoverConsumerError>
    // where
    //     RP: EventHubsRetryPolicy + Send,
    // {
    //     log::debug!("Recovering consumer");

    //     let endpoint = consumer.endpoint.to_string();
    //     let resource = endpoint.clone();
    //     let required_claims = vec![event_hub_claim::LISTEN.to_string()];
    //     self.connection_scope
    //         .request_refreshable_authorization_using_cbs(
    //             consumer.link_identifier,
    //             endpoint,
    //             resource,
    //             required_claims,
    //         )
    //         .await?;

    //     if let Some(Ok(event_position)) = consumer
    //         .current_event_position
    //         .clone()
    //         .map(|p| amqp_filter::build_filter_expression(&p))
    //     {
    //         let consumer_filter = Described::<Value>::from(ConsumerFilter(event_position));
    //         let source = consumer.receiver.source_mut().get_or_insert(
    //             Source::builder()
    //                 .address(consumer.endpoint.to_string())
    //                 .build(),
    //         );
    //         let source_filter = source.filter.get_or_insert(FilterSet::new());
    //         source_filter.insert(
    //             amqp_filter::CONSUMER_FILTER_NAME.into(),
    //             consumer_filter.into(),
    //         );
    //     }

    //     let mut exchange = if consumer.session_handle.is_ended() {
    //         let new_session = self.connection_scope.connection.begin_session().await?;
    //         let exchange = consumer
    //             .receiver
    //             .detach_then_resume_on_session(&new_session)
    //             .await?;
    //         let mut old_session = std::mem::replace(&mut consumer.session_handle, new_session);
    //         let _ = old_session.end().await;
    //         exchange
    //     } else {
    //         consumer
    //             .receiver
    //             .detach_then_resume_on_session(&consumer.session_handle)
    //             .await?
    //     };

    //     // `ReceiverAttachExchange::Complete` => Resume is complete
    //     //
    //     // `ReceiverAttachExchange::IncompleteUnsettled` => There are unsettled messages, multiple
    //     // detach and re-attach may happen in order to reduce the number of unsettled messages.
    //     //
    //     // `ReceiverAttachExchange::Resume` => There is one message that is partially transferred,
    //     // so it would be OK to let the user use the receiver to receive the message
    //     while let ReceiverAttachExchange::IncompleteUnsettled = exchange {
    //         match consumer.receiver.recv::<Body<Value>>().await {
    //             Ok(delivery) => {
    //                 let modified = Modified {
    //                     delivery_failed: None,
    //                     undeliverable_here: None,
    //                     message_annotations: None,
    //                 };
    //                 if let Err(err) = consumer.receiver.modify(delivery, modified).await {
    //                     log::error!("Failed to abandon message: {}", err);
    //                     exchange = consumer
    //                         .receiver
    //                         .detach_then_resume_on_session(&consumer.session_handle)
    //                         .await?;
    //                 }
    //             }
    //             Err(err) => {
    //                 log::error!("Failed to receive message while trying to settle (abandon) the unsettled: {}", err);
    //                 exchange = consumer
    //                     .receiver
    //                     .detach_then_resume_on_session(&consumer.session_handle)
    //                     .await?;
    //             }
    //         }
    //     }

    //     log::debug!("Consumer recovered");

    //     Ok(())
    // }

    async fn close(&mut self) -> Result<(), Self::DisposeError> {
        self.connection_scope.close().await
    }

    async fn close_if_owned(&mut self) -> Result<(), Self::DisposeError> {
        self.connection_scope.close_if_owned().await
    }

    fn is_owned(&self) -> bool {
        self.connection_scope.is_owned()
    }

    fn is_shared(&self) -> bool {
        self.connection_scope.is_shared()
    }
}

impl RecoverableTransport for AmqpClient {
    type RecoverError = RecoverTransportClientError;

    async fn recover(&mut self) -> Result<(), Self::RecoverError> {
        log::debug!("Recovering client");

        self.connection_scope.recover().await?;
        match &mut self.management_link {
            Sharable::Owned(link) => {
                link.recover(&mut self.connection_scope).await?;
            }
            Sharable::Shared(lock) => {
                let mut link = lock.write().await;
                link.recover(&mut self.connection_scope).await?;
            }
            Sharable::None => {}
        }

        log::debug!("Client recovered");

        Ok(())
    }
}
