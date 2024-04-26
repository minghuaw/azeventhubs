use url::Url;

use crate::EventHubsTransportType;

/// Errors that can occur when building the signature authorization resource for connection.
#[derive(Debug, thiserror::Error)]
pub enum BuildResourceError {
    /// Unable to parse the URL for builder.
    #[error(transparent)]
    ParseError(#[from] url::ParseError),

    /// Unable to set port to None
    #[error("Unable to set port to None")]
    SetPortError,

    /// Unable to set password to None
    #[error("Unable to set password to None")]
    SetPasswordError,

    /// Unable to set username to empty string
    #[error("Unable to set username to empty string")]
    SetUsernameError,
}

/// Builds the fully-qualified identifier for the connection, for use with signature-based
/// authorization.
pub fn build_connection_signature_authorization_resource(
    transport_type: EventHubsTransportType,
    fully_qualified_namespace: Option<&str>,
    event_hub_name: Option<&str>,
) -> Result<String, BuildResourceError> {
    let fqn = match fully_qualified_namespace {
        // If there is no namespace, there is no basis for a URL and the
        // resource is empty.
        Some("") => return Ok(String::new()),
        None => return Ok(String::new()),
        Some(fqn) => fqn,
    };

    let mut builder = Url::parse(&format!("{}://{}", transport_type.url_scheme(), fqn))?;
    builder.set_path(event_hub_name.unwrap_or_default());
    builder
        .set_port(None)
        .map_err(|_| BuildResourceError::SetPortError)?;
    builder.set_fragment(None);
    builder
        .set_password(None)
        .map_err(|_| BuildResourceError::SetPasswordError)?;
    builder
        .set_username("")
        .map_err(|_| BuildResourceError::SetUsernameError)?;

    // Removes the trailing slash if and only if there is one and it is not the first
    // character
    builder
        .path_segments_mut()
        .map_err(|_| url::ParseError::RelativeUrlWithCannotBeABaseBase)?
        .pop_if_empty();

    Ok(builder.to_string().to_lowercase())
}
