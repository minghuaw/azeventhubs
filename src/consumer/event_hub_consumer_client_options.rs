use crate::{ConnectionOptions, RetryOptions};

/// The set of options that can be specified when creating an
/// [`crate::consumer::ConsumerClient`] to configure its behavior.
#[derive(Debug, PartialEq, Eq, Clone, Default, Hash)]
pub struct ConsumerClientOptions {
    /// The set of options that can be specified when creating an Event Hub connection.
    pub connection_options: ConnectionOptions,

    /// The set of options that can be specified when retrying operations.
    pub retry_options: RetryOptions,

    /// The identifier of the consumer. If not specified, a UUID will be generated.
    pub identifier: Option<String>,
}
