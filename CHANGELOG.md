# Change log

## 0.20.0

1. Updated dependencies
   1. `base64` to "0.22"
   2. `azure_core` to "0.20"
   3. `fe2o3-amqp` to "0.10"
   4. `fe2o3-amqp-types` to "0.10"
   5. `fe2o3-amqp-management` to "0.10"
   6. `fe2o3-amqp-cbs` to "0.10"
   7. `serde_amqp` to "0.10"
   8. `fe2o3-amqp-ws` to "0.10"

## 0.19.4

1. Updated depdencies
   1. `fe2o3-amqp` to "0.8.27"
   2. `fe2o3-amqp-types` to "0.7.2"
   3. `fe2o3-amqp-management` to "0.2.3"
   4. `serde_amqp` to "0.5.10"
2. Use explicit `OrderedMap::swap_remove` instead of the deprecated `OrderedMap::remove`

## 0.19.3

1. Removed `async-trait` to use async fn in trait.

## 0.19.2

1. Ported "0.18.5"

## 0.18.5

1. Changed consumer recovery mechanism to always recreate a new AMQP receiver link instead of trying
   to resume the old link on a new session.

## 0.18.4

1. Added more debug logs for CBS recovery process

## 0.19.1

1. Fixed #32. The `is_inclusive` field in `EventPosition` is now only changed to false if it is
   cloned from the current event position.

## 0.19.0

1. Updated `azure_core` to `0.19.0`

## 0.18.3

1. A closed link will be recovered by creating a completely new link instead of trying to resume the old link.
2. Added more debug logs

## 0.18.2

1. Added more debug logs
2. Perform management client recovery anyway even if the management link session is still active
   (currently there is no guarantee that the session will be closed when the link is closed, and
    there is no cheap way to check if the management link is still active)

## 0.18.1

1. Closes the old session and management link when recovering from a retryable error.

## 0.18.0

1. Updated `azure_core` to `0.18.0`
2. Removed depcrecated methods
3. Fixed a bug caused by that AMQP management link cannot resume on a new session.
4. Creating producer and consnumer will also follow the retry policy

## ~~0.17.2~~

1. Creating producer and consnumer will also follow the retry policy

## ~~0.17.1~~

1. Fixed a bug caused by that AMQP management link cannot resume on a new session.

## 0.17.0

1. Updated `azure_core` to `0.17.0`

## 0.16.0

1. Migrated to new [github repo](https://github.com/minghuaw/azeventhubs)
2. Updated `azure_core` to `0.16.0`

## 0.15.1

- Updated `fe2o3-amqp-ws` to "0.4.0" which comes with an upstream fix of
  [CVE-2023-43669](https://github.com/snapview/tungstenite-rs/pull/379).

## 0.15.0

- Updated `azure_core` to `0.15.0`

### Breaking changes

- The generic parameter of `EventStream` now only carries the retry policy. Private types like
  `AmqpConsumer` and `MultipleAmqpConsumer` are hidden in an internal `enum`.
- Removed the deprecated fields `cache_event_count` and `max_wait_time` from `ReadEventOptions`
- The `MaxRetries` type (used in `EventHubsRetryOptions`) can only be constructed via
  `TryFrom::try_from()` or `new()` which is just an alias for `try_from()`

## 0.14.4

- Removed the internal buffer from `EventStream`
- Deprecates `cache_event_count` and `max_wait_time` fields in `ReadEventOptions`

## 0.14.3

- Reworked `EventHubConnection` to have more fine-grained locks
- Changed `EventStream` error type to `azure_core::Error`
- Added type alias `SingleConsumerEventStream` and `MultiConsumerEventStream`

## 0.14.2

- Exposed `TryAddError` to public

## 0.14.1

- Renamed the following constructor methods and marked the old methods as deprecated
  - `from_connection_string()` -> `new_from_connection_string()`
  - `from_namespace_and_credential()` -> `new_from_credential()`
  - `from_namespace_and_named_key_credential()` -> `new_from_named_key_credential()`
  - `from_namespace_and_sas_credential()` -> `new_from_sas_credential()`
- Removed generic parameter `C` from `EventHubConnection`

## 0.14.0

- Fixed problem with `azure_identity` credentials
- Added example showcasing how to work with `azure_identity` credentials

## 0.14.0-alpha

- Changed version number to follow that of `azure_core`
- Changing visibility of the following to public
  - `BasicRetryPolicy`,
  - `mod authorization`,
  - `EventHubTokenCredential`
  - `SharedAccessCredential`
  - `AzureNamedKeyCredential`
  - `AzureSasCredential`
- Added helper function `crate::authorization::build_connection_signature_authorization_resource()`
- Added following convenience constructor methods to `EventHubConnection`, `EventHubProducerClient`, and `EventHubConsumerClient`
  - `from_namespace_and_named_key_credential()`
  - `from_namespace_and_sas_credential()`

## 0.1.2

### 0.1.2-beta

- Changed visibility of `IntoAzureCoreError` to `pub(crate)` and restricted its impl to only foreign
  error types. All other error type natively implements `Into<azure_core::error::Error>`

### 0.1.2-alpha

- Fixed a bug where `EventStream` is not `Send` because `ClosingBoxedFuture` misses `Send` in its
  trait bounds
- Changed visibility of struct `EventStream` to public
- Changed visibility of trait `IntoAzureCoreError` to public
- Updated dependency `azure_core` to `"0.13"`
- Updated dependency `time`'s version to `"<=0.3.23"`, which is the latest version that supports
  rust version 1.65

## 0.1.1

- Fixed wrong crate name in the example

## 0.1.0

- Initial release
