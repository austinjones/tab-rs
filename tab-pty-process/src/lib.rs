pub mod unix;

use std::{io, process::ExitStatus};

use async_trait::async_trait;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Error, Debug)]
pub enum PtySystemError {
    #[error("io error: {0}")]
    IoError(io::Error),
}

pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

pub struct PtySystemOptions {
    pub raw_mode: bool,
}

impl Default for PtySystemOptions {
    fn default() -> Self {
        Self { raw_mode: false }
    }
}

#[async_trait]
pub trait Child {
    async fn wait(self) -> io::Result<ExitStatus>;

    async fn kill(&mut self) -> io::Result<()>;
}

#[async_trait]
pub trait Master {
    async fn size(&self) -> io::Result<Size>;
    async fn resize(&self, size: Size) -> io::Result<()>;
}

pub trait PtySystem {
    type Child: Child;
    type Master: Master;
    type MasterRead: AsyncRead;
    type MasterWrite: AsyncWrite;

    fn spawn(
        command: tokio::process::Command,
        options: PtySystemOptions,
    ) -> Result<PtySystemInstance<Self>, PtySystemError>;
}

pub struct PtySystemInstance<P>
where
    P: PtySystem + ?Sized,
{
    pub child: P::Child,
    pub master: P::Master,
    pub read: P::MasterRead,
    pub write: P::MasterWrite,
}

pub trait MasterRead: AsyncRead {}

pub trait MasterWrite: AsyncWrite {}
