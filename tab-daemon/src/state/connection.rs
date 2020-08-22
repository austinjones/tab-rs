#[derive(Clone, Debug)]
pub struct ConnectionState {
    pub auth: bool,
}

impl Default for ConnectionState {
    fn default() -> Self {
        ConnectionState { auth: false }
    }
}

impl ConnectionState {}
