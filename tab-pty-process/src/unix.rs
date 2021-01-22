mod child;
mod internal;
// mod pty_file;

use async_trait::async_trait;

use std::{
    io, mem,
    os::unix::prelude::AsRawFd,
    pin::Pin,
    sync::{Arc, Mutex},
};

use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    process::Command,
};

use crate::{Master, PtySystem, PtySystemError, PtySystemInstance, PtySystemOptions, Size};

use self::{child::UnixPtyChild, internal::UnixInternal};

pub struct UnixPtySystem {}

impl PtySystem for UnixPtySystem {
    type Child = UnixPtyChild;
    type Master = UnixPtyMaster;
    type MasterRead = UnixPtyRead;
    type MasterWrite = UnixPtyWrite;

    fn spawn(
        mut command: Command,
        options: PtySystemOptions,
    ) -> Result<crate::PtySystemInstance<Self>, crate::PtySystemError> {
        let internal = UnixInternal::new().map_err(|e| PtySystemError::IoError(e))?;
        let master_fd = internal.as_raw_fd();

        let slave_fd = {
            let slave = internal
                .open_sync_pty_slave()
                .map_err(|e| PtySystemError::IoError(e))?;
            let slave_fd = slave.as_raw_fd();

            let stdin = slave.try_clone().map_err(|e| PtySystemError::IoError(e))?;
            command.stdin(stdin);
            let stdout = slave.try_clone().map_err(|e| PtySystemError::IoError(e))?;
            command.stdout(stdout);
            command.stderr(slave);

            slave_fd
        };

        let internal = Arc::new(Mutex::new(internal));

        let master = UnixPtyMaster(internal.clone());
        let read = UnixPtyRead(internal.clone());
        let write = UnixPtyWrite(internal.clone());

        unsafe {
            command.pre_exec(move || {
                if options.raw_mode {
                    let mut attrs: libc::termios = mem::zeroed();

                    if libc::tcgetattr(slave_fd, &mut attrs as _) != 0 {
                        return Err(io::Error::last_os_error());
                    }

                    libc::cfmakeraw(&mut attrs as _);

                    if libc::tcsetattr(slave_fd, libc::TCSANOW, &attrs as _) != 0 {
                        return Err(io::Error::last_os_error());
                    }
                }

                // This is OK even though we don't own master since this process is
                // about to become something totally different anyway.
                if libc::close(master_fd) != 0 {
                    return Err(io::Error::last_os_error());
                }

                if libc::setsid() < 0 {
                    return Err(io::Error::last_os_error());
                }

                if libc::ioctl(0, libc::TIOCSCTTY.into(), 1) != 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }

        let child = command.spawn().map_err(|e| PtySystemError::IoError(e))?;
        let child = UnixPtyChild::new(child);

        Ok(PtySystemInstance {
            child,
            master,
            read,
            write,
        })
    }
}

pub struct UnixPtyMaster(Arc<Mutex<UnixInternal>>);

#[async_trait]
impl Master for UnixPtyMaster {
    async fn size(&self) -> std::io::Result<crate::Size> {
        let lock = self.0.lock().unwrap();
        let (cols, rows) = lock.winsize()?;
        Ok(Size { cols, rows })
    }

    async fn resize(&self, size: crate::Size) -> std::io::Result<()> {
        let lock = self.0.lock().unwrap();
        lock.resize(size.cols, size.rows)
    }
}

pub struct UnixPtyRead(Arc<Mutex<UnixInternal>>);

impl AsyncRead for UnixPtyRead {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let mut lock = self.0.lock().unwrap();
        AsyncRead::poll_read(Pin::new(&mut *lock), cx, buf)
    }
}

pub struct UnixPtyWrite(Arc<Mutex<UnixInternal>>);

impl AsyncWrite for UnixPtyWrite {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, io::Error>> {
        let mut lock = self.0.lock().unwrap();
        AsyncWrite::poll_write(Pin::new(&mut *lock), cx, buf)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        let mut lock = self.0.lock().unwrap();
        AsyncWrite::poll_flush(Pin::new(&mut *lock), cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        let mut lock = self.0.lock().unwrap();
        AsyncWrite::poll_shutdown(Pin::new(&mut *lock), cx)
    }
}
