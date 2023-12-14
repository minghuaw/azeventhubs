use fe2o3_amqp::{session::SessionHandle, link::{SendError, LinkStateError}};
use fe2o3_amqp_management::{MgmtClient, error::Error as ManagementError, Request, Response};
use fe2o3_amqp_types::messaging::FromBody;

use crate::util::sharable::Sharable;

use super::{amqp_connection_scope::AmqpConnectionScope, error::OpenMgmtLinkError};

#[derive(Debug)]
enum State {
    Recovering,
    Connected {
        session: SessionHandle<()>,
        client: MgmtClient,
    }
}

#[derive(Debug)]
pub(crate) struct AmqpManagementLink {
    state: State,
}

impl AmqpManagementLink {
    pub(crate) fn new(session: SessionHandle<()>, client: MgmtClient) -> Self {
        let state = State::Connected {
            session,
            client,
        };
        Self {
            state
        }
    }

    pub(crate) async fn call<Req, Res>(&mut self, request: Req) -> Result<Res, ManagementError> 
    where
        Req: Request<Response = Res>,
        Res: Response,
        Res::Error: Into<ManagementError>,
        for<'de> Res::Body: FromBody<'de> + std::fmt::Debug + Send,
    {
        match &mut self.state {
            State::Recovering => {
                Err(ManagementError::Send(SendError::LinkStateError(LinkStateError::IllegalSessionState)))
            },
            State::Connected { client, .. } => {
                client.call(request).await
            }
        }
    }

    pub(crate) async fn recover(&mut self, scope: &mut AmqpConnectionScope) -> Result<(), OpenMgmtLinkError> {
        log::debug!("Recovering management link");

        // Close the old session and client
        let old_state = std::mem::replace(&mut self.state, State::Recovering);
        if let State::Connected { mut session, client } = old_state {
            if let Err(err) = client.close().await {
                log::error!("Found error closing management client during recovery: {:?}", err);
            }
            if let Err(err) = session.end().await {
                log::error!("Found error closing management session during recovery: {:?}", err);
            }
        }

        let (new_session, new_client) = scope.create_management_link().await?;
        let new_state = State::Connected {
            session: new_session,
            client: new_client,
        };
        let _ = std::mem::replace(&mut self.state, new_state);

        Ok(())
    }
}

impl Sharable<AmqpManagementLink> {
    pub(crate) async fn call<Req, Res>(&mut self, request: Req) -> Result<Res, ManagementError>
    where
        Req: Request<Response = Res>,
        Res: Response,
        Res::Error: Into<ManagementError>,
        for<'de> Res::Body: FromBody<'de> + std::fmt::Debug + Send,
    {
        match self {
            Sharable::Owned(l) => l.call(request).await,
            Sharable::Shared(lock) => {
                let mut lock = lock.write().await;
                lock.call(request).await
            },
            Sharable::None => unreachable!(),
        }
    }
}
