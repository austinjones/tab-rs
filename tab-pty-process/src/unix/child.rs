use std::{io, process::ExitStatus};

use crate::Child;
use async_trait::async_trait;
use tokio::process::Child as TokioChild;

/// A child process that can be interacted with through a pseudo-TTY.
pub struct UnixPtyChild(TokioChild);

impl UnixPtyChild {
    pub fn new(inner: TokioChild) -> Self {
        Self(inner)
    }

    // Returns the OS-assigned process identifier associated with this child.
    // pub fn id(&self) -> u32 {
    //     self.0.id()
    // }
}

#[async_trait]
impl Child for UnixPtyChild {
    async fn wait(mut self) -> io::Result<ExitStatus> {
        self.0.wait().await
    }

    async fn kill(&mut self) -> io::Result<()> {
        self.0.kill().await
    }
}
