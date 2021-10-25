use crate::{message::listener::RequestMetadata, resource::listener::WebsocketAuthToken};
use tungstenite::handshake::server::{Callback, ErrorResponse, Request, Response};

use lifeline::request::Request as LifelineRequest;

/// A tungstenite handler that rejects origin headers,
///  requires auth tokens (if the token resource contains a Some value),
///  and collects the request metadata.
pub struct AuthHandler {
    token: WebsocketAuthToken,
    send_metadata: Option<LifelineRequest<(), RequestMetadata>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthState {
    /// The connection request was valid, and accepted.
    Ok,
    /// The connection request was invalid, as it contained an Origin header (and thus came from a browser)
    RejectOrigin,
    /// The connection request was invalid, as it did not contain the required authentication token.
    RejectAuth,
}

impl AuthHandler {
    #[cfg(test)]
    pub fn new(token: WebsocketAuthToken) -> Self {
        AuthHandler {
            token,
            send_metadata: None,
        }
    }

    pub fn with_metadata(
        token: WebsocketAuthToken,
        send_metadata: Option<LifelineRequest<(), RequestMetadata>>,
    ) -> Self {
        AuthHandler {
            token,
            send_metadata,
        }
    }

    fn response_forbidden() -> ErrorResponse {
        Response::builder()
            .status(403)
            .body(Some("Forbidden".to_string()))
            .expect("forbidden response")
    }

    fn response_unauthorized() -> ErrorResponse {
        Response::builder()
            .status(401)
            .body(Some("Unauthorized".to_string()))
            .expect("unauthorized response")
    }

    pub fn validate_token(&self, request: &Request) -> AuthState {
        if request.headers().get("origin").is_some() {
            return AuthState::RejectOrigin;
        }

        if self.token.0.is_none() {
            return AuthState::Ok;
        }

        let expected_token = self.token.0.as_ref().unwrap().as_str();

        if request.headers().get("authorization").is_none() {
            return AuthState::RejectAuth;
        }

        let token = request.headers().get("authorization").unwrap().to_str();
        if let Err(_e) = token {
            return AuthState::RejectAuth;
        }

        let provided_token = token.unwrap().trim();
        if expected_token == provided_token {
            AuthState::Ok
        } else {
            AuthState::RejectAuth
        }
    }
}

impl Callback for AuthHandler {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        let result = match self.validate_token(request) {
            AuthState::Ok => Ok(response),
            AuthState::RejectOrigin => Err(Self::response_forbidden()),
            AuthState::RejectAuth => Err(Self::response_unauthorized()),
        };

        if let Some(send_metadata) = self.send_metadata {
            let uri = request.uri().clone();
            let method = request.method().clone();
            let metadata = RequestMetadata { method, uri };
            tokio::spawn(send_metadata.reply(|_r| async { metadata }));
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthHandler, AuthState};
    use tungstenite::handshake::server::Request;

    #[test]
    fn validate_rejects_no_token() -> anyhow::Result<()> {
        let auth = AuthHandler::new("token".into());

        let request = Request::builder().body(())?;

        assert_eq!(AuthState::RejectAuth, auth.validate_token(&request));

        Ok(())
    }

    #[test]
    fn validate_rejects_bad_token() -> anyhow::Result<()> {
        let auth = AuthHandler::new("token".into());

        let request = Request::builder().body(())?;

        assert_eq!(AuthState::RejectAuth, auth.validate_token(&request));

        Ok(())
    }
}
