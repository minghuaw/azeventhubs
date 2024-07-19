use std::borrow::Cow;
use fe2o3_amqp_types::{
    messaging::{
        annotations::AnnotationKey, Message, MessageId, Properties,
    },
    primitives::{BinaryRef, Symbol, Value},
};

use crate::constants::MAX_MESSAGE_ID_LENGTH;

use super::{
    amqp_property,
    error::{MaxLengthExceededError, SetMessageIdError},
};

pub(crate) trait AmqpMessageExt {
    fn message_id(&self) -> Option<Cow<'_, str>>;

    fn partition_key(&self) -> Option<&str>;

    fn correlation_id(&self) -> Option<Cow<'_, str>>;

    fn content_type(&self) -> Option<&str>;
}

pub(crate) trait AmqpMessageMutExt {
    fn set_message_id(&mut self, message_id: impl Into<String>) -> Result<(), SetMessageIdError>;

    fn set_correlation_id(&mut self, id: impl Into<Option<String>>);

    fn set_content_type(&mut self, content_type: impl Into<Option<String>>);
}

impl<B> AmqpMessageExt for Message<B> {
    #[inline]
    fn message_id(&self) -> Option<Cow<'_, str>> {
        match self.properties.as_ref()?.message_id.as_ref()? {
            MessageId::String(val) => Some(Cow::Borrowed(val)),
            MessageId::Ulong(val) => Some(Cow::Owned(val.to_string())),
            MessageId::Uuid(uuid) => Some(Cow::Owned(format!("{:x}", uuid))),
            MessageId::Binary(bytes) => {
                let binary_ref = BinaryRef::from(bytes);
                Some(Cow::Owned(format!("{:X}", binary_ref)))
            }
        }
    }

    #[inline]
    fn partition_key(&self) -> Option<&str> {
        self.message_annotations
            .as_ref()?
            .get(&amqp_property::PARTITION_KEY as &dyn AnnotationKey)
            .map(|value| match value {
                Value::String(s) => s.as_str(),
                _ => unreachable!("Expecting a String"),
            })
    }

    #[inline]
    fn correlation_id(&self) -> Option<Cow<'_, str>> {
        match self.properties.as_ref()?.correlation_id.as_ref()? {
            MessageId::String(val) => Some(Cow::Borrowed(val)),
            MessageId::Ulong(val) => Some(Cow::Owned(val.to_string())),
            MessageId::Uuid(uuid) => Some(Cow::Owned(format!("{:x}", uuid))),
            MessageId::Binary(bytes) => {
                let binary_ref = BinaryRef::from(bytes);
                Some(Cow::Owned(format!("{:X}", binary_ref)))
            }
        }
    }

    #[inline]
    fn content_type(&self) -> Option<&str> {
        self.properties
            .as_ref()?
            .content_type
            .as_ref()
            .map(|s| s.as_str())
    }

}

impl<B> AmqpMessageMutExt for Message<B> {
    /// Returns `Err(_)` if message_id length exceeds [`MAX_MESSAGE_ID_LENGTH`]
    #[inline]
    fn set_message_id(&mut self, message_id: impl Into<String>) -> Result<(), SetMessageIdError> {
        let message_id = message_id.into();

        if message_id.is_empty() {
            return Err(SetMessageIdError::Empty);
        }

        if message_id.len() > MAX_MESSAGE_ID_LENGTH {
            return Err(
                MaxLengthExceededError::new(message_id.len(), MAX_MESSAGE_ID_LENGTH).into(),
            );
        }

        self.properties
            .get_or_insert(Properties::default())
            .message_id = Some(MessageId::String(message_id));
        Ok(())
    }

    #[inline]
    fn set_correlation_id(&mut self, id: impl Into<Option<String>>) {
        let correlation_id = id.into().map(MessageId::String);
        self.properties
            .get_or_insert(Properties::default())
            .correlation_id = correlation_id;
    }

    #[inline]
    fn set_content_type(&mut self, content_type: impl Into<Option<String>>) {
        let content_type = content_type.into();
        self.properties
            .get_or_insert(Properties::default())
            .content_type = content_type.map(Symbol::from);
    }
}
