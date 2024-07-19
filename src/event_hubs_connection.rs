use const_format::concatcp;
use std::sync::Arc;
use url::Url;

use crate::{
    amqp::{
        amqp_client::AmqpClient,
        amqp_consumer::AmqpConsumer,
        amqp_producer::AmqpProducer,
        error::AmqpClientError,
    },
    authorization::{
        event_hub_token_credential::EventHubTokenCredential,
        shared_access_credential::SharedAccessCredential,
        shared_access_signature::SharedAccessSignature, AzureNamedKeyCredential,
        AzureSasCredential,
    },
    consumer::EventPosition,
    core::{RecoverableTransport, TransportClient, TransportProducerFeatures, RecoverableError},
    event_hubs_connection_option::ConnectionOptions,
    event_hubs_connection_string_properties::ConnectionStringProperties,
    event_hubs_properties::Properties,
    event_hubs_retry_policy::EventHubsRetryPolicy,
    event_hubs_transport_type::TransportType,
    producer::PartitionPublishingOptions,
    PartitionProperties, util,
};

/// Error with the `Connection`.
#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    /// The event hub name is not specified.
    #[error("The EventHub name is not specified")]
    EventHubNameIsNotSpecified,
}

impl From<ConnectionError> for azure_core::error::Error {
    fn from(error: ConnectionError) -> Self {
        use azure_core::error::ErrorKind;

        azure_core::Error::new(ErrorKind::Other, error)
    }
}

/// A connection to the Azure Event Hubs service, enabling client communications with a specific
/// Event Hub instance within an Event Hubs namespace.  A single connection may be shared among multiple
/// Event Hub producers and/or consumers, or may be used as a dedicated connection for a single
/// producer or consumer client.
#[derive(Debug)]
pub struct Connection {
    fully_qualified_namespace: Arc<String>,
    event_hub_name: Arc<String>,
    pub(crate) inner: AmqpClient,
}

impl Connection {
    /// Creates a new [`Connection`] from a connection string.
    pub async fn new_from_connection_string(
        connection_string: impl AsRef<str>,
        event_hub_name: impl Into<Option<String>>,
        options: ConnectionOptions,
    ) -> Result<Self, azure_core::Error> {
        let connection_string_properties =
            ConnectionStringProperties::parse(connection_string.as_ref())?;

        let event_hub_name =
            match event_hub_name
                .into()
                .and_then(|s| if s.is_empty() { None } else { Some(s) })
            {
                None => connection_string_properties
                    .event_hub_name
                    .map(|s| s.to_string())
                    .ok_or(ConnectionError::EventHubNameIsNotSpecified)?,
                Some(s) => s,
            };

        macro_rules! ok_if_not_none_or_empty {
            ($id:expr, $type_name:literal) => {
                match $id {
                    Some(s) if s.is_empty() => Err(azure_core::Error::new(
                        azure_core::error::ErrorKind::Credential,
                        concatcp!("{} cannot be empty", $type_name),
                    )),
                    None => Err(azure_core::Error::new(
                        azure_core::error::ErrorKind::Credential,
                        concatcp!("{} cannot be None", $type_name),
                    )),
                    Some(s) => Ok(s),
                }
            };
        }

        let fully_qualified_namespace = connection_string_properties
            .fully_qualified_namespace()
            .ok_or_else(|| {
                azure_core::Error::new(
                    azure_core::error::ErrorKind::Credential,
                    "fully_qualified_namespace cannot be None",
                )
            })?;

        let shared_access_signature = if let Some(shared_access_signature) =
            connection_string_properties.shared_access_signature
        {
            SharedAccessSignature::try_from_signature(shared_access_signature)?
        } else {
            let resource = build_connection_signature_authorization_resource(
                options.transport_type,
                fully_qualified_namespace,
                &event_hub_name,
            )?;
            let shared_access_key_name = ok_if_not_none_or_empty!(
                connection_string_properties.shared_access_key_name(),
                "shared_access_key_name"
            )?;
            let shared_access_key = ok_if_not_none_or_empty!(
                connection_string_properties.shared_access_key(),
                "shared_access_key"
            )?;
            SharedAccessSignature::try_from_parts(
                resource,
                shared_access_key_name,
                shared_access_key,
                None,
            )?
        };

        let shared_access_credential =
            SharedAccessCredential::from_signature(shared_access_signature);

        let token_credential =
            EventHubTokenCredential::SharedAccessCredential(shared_access_credential);

        Self::new_from_credential(
            fully_qualified_namespace.to_string(),
            event_hub_name,
            token_credential,
            options,
        )
        .await
    }

    /// Creates a new [`Connection`] from a namespace and a credential.
    pub async fn new_from_credential(
        fully_qualified_namespace: impl Into<String>,
        event_hub_name: impl Into<String>,
        credential: impl Into<EventHubTokenCredential>,
        options: ConnectionOptions,
    ) -> Result<Self, azure_core::Error> {
        let fully_qualified_namespace = fully_qualified_namespace.into();
        let event_hub_name = event_hub_name.into();
        let token_credential = credential.into();
        let event_hub_name = Arc::new(event_hub_name);

        let inner_client = AmqpClient::new(
            &fully_qualified_namespace,
            event_hub_name.clone(),
            token_credential,
            options,
        )
        .await
        .map_err(<AmqpClientError as Into<azure_core::Error>>::into)?;

        let fully_qualified_namespace = Arc::new(fully_qualified_namespace);
        Ok(Self {
            fully_qualified_namespace,
            event_hub_name,
            inner: inner_client,
        })
    }

    /// Creates a new [`Connection`] from a namespace and a [`AzureNamedKeyCredential`].
    pub async fn new_from_named_key_credential(
        fully_qualified_namespace: impl Into<String>,
        event_hub_name: impl Into<String>,
        credential: AzureNamedKeyCredential,
        options: ConnectionOptions,
    ) -> Result<Self, azure_core::Error> {
        let fully_qualified_namespace = fully_qualified_namespace.into();
        let event_hub_name = event_hub_name.into();
        let resource = build_connection_signature_authorization_resource(
            options.transport_type,
            &fully_qualified_namespace,
            &event_hub_name,
        )?;
        let shared_access_credential =
            SharedAccessCredential::try_from_named_key_credential(credential, resource)?;

        Self::new_from_credential(
            fully_qualified_namespace,
            event_hub_name,
            shared_access_credential,
            options,
        )
        .await
    }

    /// Creates a new [`Connection`] from a namespace and a [`AzureSasCredential`].
    pub async fn new_from_sas_credential(
        fully_qualified_namespace: impl Into<String>,
        event_hub_name: impl Into<String>,
        credential: AzureSasCredential,
        options: ConnectionOptions,
    ) -> Result<Self, azure_core::Error> {
        let shared_access_credential = SharedAccessCredential::try_from_sas_credential(credential)?;
        Self::new_from_credential(
            fully_qualified_namespace,
            event_hub_name,
            shared_access_credential,
            options,
        )
        .await
    }
}

impl Connection {
    pub(crate) async fn get_properties<RP>(
        &mut self,
        retry_policy: RP,
    ) -> Result<Properties, azure_core::Error>
    where
        RP: EventHubsRetryPolicy + Send,
    {
        // // We don't need to explicitly check if the connection is closed here because
        // // `self.inner.get_properties` will do that for us.
        // self.inner.get_properties(retry_policy).await

        let mut try_timeout = retry_policy.calculate_try_timeout(0);
        let mut failed_attempt = 0;
        let mut should_try_recover = false;

        loop {
            // The underlying AMQP client may get closed if idle for too long.  If that happens, we
            // need to recreate it.
            if should_try_recover {
                self.inner.recover().await?;
            }

            let fut = self.inner.get_properties();
            let error = match util::time::timeout(try_timeout, fut).await {
                Ok(Ok(response)) => return Ok(response),
                Ok(Err(err)) => err,
                Err(elapsed) => elapsed.into(),
            };

            log::debug!("get_properties failed: {:?}", error);

            failed_attempt += 1;
            let delay = retry_policy.calculate_retry_delay(&error, failed_attempt);
            should_try_recover = error.should_try_recover();
            match delay {
                Some(delay) => {
                    util::time::sleep(delay).await;
                    try_timeout = retry_policy.calculate_try_timeout(failed_attempt);
                }
                // Stop retrying and close the client. The connection close error is often more
                // useful
                None => match self.inner.close_if_owned().await {
                    Ok(_) => return Err(error.into()),
                    Err(dispose_err) => return Err(dispose_err.into()),
                },
            }
        }
    }

    pub(crate) async fn get_partition_ids<RP>(
        &mut self,
        retry_policy: RP,
    ) -> Result<Vec<String>, azure_core::Error>
    where
        RP: EventHubsRetryPolicy + Send,
    {
        // We don't need to explicitly check if the connection is closed here because
        // `self.inner.get_properties` will do that for us.
        let properties = self.get_properties(retry_policy).await?;
        Ok(properties.partition_ids)
    }

    pub(crate) async fn get_partition_properties<RP>(
        &mut self,
        partition_id: &str,
        retry_policy: RP,
    ) -> Result<PartitionProperties, azure_core::Error>
    where
        RP: EventHubsRetryPolicy + Send,
    {
        let mut try_timeout = retry_policy.calculate_try_timeout(0);
        let mut failed_attempt = 0;
        let mut should_try_recover = false;

        loop {
            // The underlying AMQP client may get closed if idle for too long.  If that happens, we
            // need to recreate it.
            if should_try_recover {
                self.inner.recover().await?;
            }

            let fut = self.inner.get_partition_properties(partition_id);
            let error = match util::time::timeout(try_timeout, fut).await {
                Ok(Ok(response)) => return Ok(response),
                Ok(Err(err)) => err,
                Err(elapsed) => elapsed.into(),
            };

            log::debug!("get_partition_properties failed: {:?}", error);

            failed_attempt += 1;
            let delay = retry_policy.calculate_retry_delay(&error, failed_attempt);
            should_try_recover = error.should_try_recover();
            match delay {
                Some(delay) => {
                    util::time::sleep(delay).await;
                    try_timeout = retry_policy.calculate_try_timeout(failed_attempt);
                }
                // Stop retrying and close the client. The connection close error is often more
                // useful
                None => match self.inner.close_if_owned().await {
                    Ok(_) => return Err(error.into()),
                    Err(dispose_err) => return Err(dispose_err.into()),
                },
            }
        }
    }

    pub(crate) async fn create_transport_producer<RP>(
        &mut self,
        partition_id: Option<String>,
        producer_identifier: Option<String>,
        requested_features: TransportProducerFeatures,
        partition_options: PartitionPublishingOptions,
        retry_policy: RP,
    ) -> Result<AmqpProducer<RP>, azure_core::Error>
    where
        RP: EventHubsRetryPolicy + Send,
    {
        let mut try_timeout = retry_policy.calculate_try_timeout(0);
        let mut failed_attempt = 0;
        let mut should_try_recover = false;

        loop {
            // The underlying AMQP client may get closed if idle for too long.  If that happens, we
            // need to recreate it.
            if should_try_recover {
                self.inner.recover().await?;
            }

            // TODO: can we reduce clone() calls?
            let fut = self.inner
                .create_producer(
                    partition_id.clone(),
                    producer_identifier.clone(),
                    requested_features,
                    partition_options.clone(),
                    retry_policy.clone(),
                );
            let error = match util::time::timeout(try_timeout, fut).await {
                Ok(Ok(response)) => return Ok(response),
                Ok(Err(err)) => err,
                Err(elapsed) => elapsed.into(),
            };

            log::debug!("create producer failed: {:?}", error);

            failed_attempt += 1;
            let delay = retry_policy.calculate_retry_delay(&error, failed_attempt);
            should_try_recover = error.should_try_recover();
            match delay {
                Some(delay) => {
                    util::time::sleep(delay).await;
                    try_timeout = retry_policy.calculate_try_timeout(failed_attempt);
                }
                // Stop retrying and close the client. The connection close error is often more
                // useful
                None => match self.inner.close_if_owned().await {
                    Ok(_) => return Err(error.into()),
                    Err(dispose_err) => return Err(dispose_err.into()),
                },
            }
        }
    }

    #[allow(clippy::too_many_arguments)] // TODO: how to reduce the number of arguments?
    pub(crate) async fn create_transport_consumer<RP>(
        &mut self,
        consumer_group: &str,
        partition_id: &str,
        consumer_identifier: Option<String>,
        event_position: EventPosition,
        retry_policy: RP,
        track_last_enqueued_event_properties: bool,
        owner_level: Option<i64>,
        prefetch_count: Option<u32>,
    ) -> Result<AmqpConsumer<RP>, azure_core::Error>
    where
        RP: EventHubsRetryPolicy + Send,
    {
        let mut try_timeout = retry_policy.calculate_try_timeout(0);
        let mut failed_attempt = 0;
        let mut should_try_recover = false;

        loop {
            // The underlying AMQP client may get closed if idle for too long.  If that happens, we
            // need to recreate it.
            if should_try_recover {
                self.inner.recover().await?;
            }

            let fut = self
                .inner
                .create_consumer(
                    consumer_group,
                    partition_id,
                    consumer_identifier.clone(),
                    &event_position,
                    retry_policy.clone(),
                    track_last_enqueued_event_properties,
                    owner_level,
                    prefetch_count,
                );
            let error = match util::time::timeout(try_timeout, fut).await {
                Ok(Ok(response)) => return Ok(response),
                Ok(Err(err)) => err,
                Err(elapsed) => elapsed.into(),
            };

            log::debug!("create consumer failed: {:?}", error);

            failed_attempt += 1;
            let delay = retry_policy.calculate_retry_delay(&error, failed_attempt);
            should_try_recover = error.should_try_recover();
            match delay {
                Some(delay) => {
                    util::time::sleep(delay).await;
                    try_timeout = retry_policy.calculate_try_timeout(failed_attempt);
                }
                // Stop retrying and close the client. The connection close error is often more
                // useful
                None => match self.inner.close_if_owned().await {
                    Ok(_) => return Err(error.into()),
                    Err(dispose_err) => return Err(dispose_err.into()),
                },
            }
        }
    }

    /// Closes the inner client regardless of whether it is owned or shared.
    pub async fn close(mut self) -> Result<(), azure_core::Error> {
        self.inner.close().await.map_err(Into::into)
    }

    /// Closes the inner client if it is owned or if it is shared and this is the last reference to
    /// it.
    pub async fn close_if_owned(mut self) -> Result<(), azure_core::Error> {
        self.inner.close_if_owned().await.map_err(Into::into)
    }
}

impl Connection {
    pub(crate) fn clone_as_shared(&mut self) -> Self {
        Self {
            fully_qualified_namespace: self.fully_qualified_namespace.clone(),
            event_hub_name: self.event_hub_name.clone(),
            inner: self.inner.clone_as_shared(),
        }
    }

    /// The fully qualified namespace that the connection is associated with.
    pub fn fully_qualified_namespace(&self) -> &str {
        &self.fully_qualified_namespace
    }

    /// The name of the event hub that the connection is associated with.
    pub fn event_hub_name(&self) -> &str {
        &self.event_hub_name
    }

    /// Returns true if the connection is closed.
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    /// Returns true if the connection is owned.
    ///
    /// This will return false even if it is the last reference to the shared connection.
    pub fn is_owned(&self) -> bool {
        self.inner.is_owned()
    }

    /// Returns true if the connection is shared.
    ///
    /// This will return true even if it is the last reference to the shared connection.
    pub fn is_shared(&self) -> bool {
        self.inner.is_shared()
    }
}

// internal static string BuildConnectionSignatureAuthorizationResource(TransportType transportType,
//     string fullyQualifiedNamespace,
//     string eventHubName)
fn build_connection_signature_authorization_resource(
    transport_type: TransportType,
    fully_qualified_namespace: &str,
    event_hub_name: &str,
) -> Result<String, azure_core::Error> {
    use crate::event_hubs_connection_string_properties::FormatError;
    use azure_core::error::ErrorKind;

    // If there is no namespace, there is no basis for a URL and the
    // resource is empty.

    if fully_qualified_namespace.is_empty() {
        return Err(FormatError::ConnectionStringIsEmpty.into());
    }

    // Form a normalized URI to identify the resource.

    let mut builder = Url::parse(&format!(
        "{}://{}",
        transport_type.url_scheme(),
        fully_qualified_namespace
    ))?;
    builder.set_path(event_hub_name);
    builder
        .set_port(None)
        .map_err(|_| azure_core::Error::new(ErrorKind::Other, "Unable to set port to None"))?;
    builder.set_fragment(None);
    builder.set_password(None).map_err(|_| {
        azure_core::Error::new(
            ErrorKind::Other,
            "Unable to set password to None".to_string(),
        )
    })?;
    builder.set_username("").map_err(|_| {
        azure_core::Error::new(
            ErrorKind::Other,
            "Unable to set username to empty string".to_string(),
        )
    })?;

    // Removes the trailing slash if and only if there is one and it is not the first
    // character
    builder
        .path_segments_mut()
        .map_err(|_| url::ParseError::RelativeUrlWithCannotBeABaseBase)?
        .pop_if_empty();

    Ok(builder.to_string().to_lowercase())
}

impl RecoverableTransport for Connection {
    type RecoverError = azure_core::Error;

    async fn recover(&mut self) -> Result<(), Self::RecoverError> {
        self.inner.recover().await.map_err(Into::into)
    }
}
