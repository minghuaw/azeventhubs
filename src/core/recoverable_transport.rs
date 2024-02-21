pub trait RecoverableTransport {
    type RecoverError: Send;

    async fn recover(&mut self) -> Result<(), Self::RecoverError>;
}

pub trait RecoverableError {
    fn should_try_recover(&self) -> bool;

    fn is_scope_disposed(&self) -> bool;
}
