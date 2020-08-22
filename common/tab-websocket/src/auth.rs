use crate::resource::listener::WebsocketAuthToken;
use tungstenite::handshake::server::{Callback, ErrorResponse, Request, Response};

pub struct AuthHandler {
    token: WebsocketAuthToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthState {
    Ok,
    RejectOrigin,
    RejectAuth,
}

impl AuthHandler {
    pub fn new(token: WebsocketAuthToken) -> Self {
        AuthHandler { token }
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
        // AuthState::RejectOrigin
        if request.headers().get("origin").is_some() {
            return AuthState::RejectOrigin;
        }

        if self.token.0.is_none() {
            return AuthState::Ok;
        }

        let expected_token = self.token.0.as_ref().unwrap().as_str();

        if !request.headers().get("authorization").is_some() {
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
        match self.validate_token(request) {
            AuthState::Ok => Ok(response),
            AuthState::RejectOrigin => Err(Self::response_forbidden()),
            AuthState::RejectAuth => Err(Self::response_unauthorized()),
        }
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
