// Copyright 2018 Peter Williams, Tokio Contributors
// Copyright 2019 Fabian Freyer
// Licensed under both the MIT License and the Apache-2.0 license.

//! This module is a clone of
//! <https://github.com/tokio-rs/tokio/blob/master/tokio-io/src/split.rs>
//! (commit 1119d57), modified to refer to our AsyncPtyMaster types. We need
//! to implement the splitting ourselves in order to be able to implement
//! AsRawFd for the split types.

use futures::{lock::BiLock, ready};
use std::io::{self};
use std::{
    os::unix::io::{AsRawFd, RawFd},
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{AsAsyncPtyFd, AsyncPtyMaster};

pub fn split(master: AsyncPtyMaster) -> (AsyncPtyMasterReadHalf, AsyncPtyMasterWriteHalf) {
    let (a, b) = BiLock::new(master);
    (
        AsyncPtyMasterReadHalf { handle: a },
        AsyncPtyMasterWriteHalf { handle: b },
    )
}

/// Read half of a AsyncPtyMaster, created with AsyncPtyMaster::split.
pub struct AsyncPtyMasterReadHalf {
    handle: BiLock<AsyncPtyMaster>,
}

/// Write half of a AsyncPtyMaster, created with AsyncPtyMaster::split.
pub struct AsyncPtyMasterWriteHalf {
    handle: BiLock<AsyncPtyMaster>,
}

impl AsAsyncPtyFd for AsyncPtyMasterReadHalf {
    fn as_async_pty_fd(&self, cx: &mut Context<'_>) -> Poll<RawFd> {
        let l = ready!(self.handle.poll_lock(cx));
        Poll::Ready(l.as_raw_fd())
    }
}

impl AsAsyncPtyFd for &AsyncPtyMasterReadHalf {
    fn as_async_pty_fd(&self, cx: &mut Context<'_>) -> Poll<RawFd> {
        let l = ready!(self.handle.poll_lock(cx));
        Poll::Ready(l.as_raw_fd())
    }
}

impl AsAsyncPtyFd for &mut AsyncPtyMasterReadHalf {
    fn as_async_pty_fd(&self, cx: &mut Context<'_>) -> Poll<RawFd> {
        let l = ready!(self.handle.poll_lock(cx));
        Poll::Ready(l.as_raw_fd())
    }
}

impl AsAsyncPtyFd for &AsyncPtyMasterWriteHalf {
    fn as_async_pty_fd(&self, cx: &mut Context<'_>) -> Poll<RawFd> {
        let l = ready!(self.handle.poll_lock(cx));
        Poll::Ready(l.as_raw_fd())
    }
}

impl AsAsyncPtyFd for &mut AsyncPtyMasterWriteHalf {
    fn as_async_pty_fd(&self, cx: &mut Context<'_>) -> Poll<RawFd> {
        let l = ready!(self.handle.poll_lock(cx));
        Poll::Ready(l.as_raw_fd())
    }
}

impl AsyncRead for AsyncPtyMasterReadHalf {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<io::Result<usize>> {
        let mut l = ready!(self.handle.poll_lock(cx));
        l.as_pin_mut().poll_read(cx, buf)
    }
}

impl AsyncWrite for AsyncPtyMasterWriteHalf {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let mut l = ready!(self.handle.poll_lock(cx));
        l.as_pin_mut().poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        let mut l = ready!(self.handle.poll_lock(cx));
        l.as_pin_mut().poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        let mut l = ready!(self.handle.poll_lock(cx));
        l.as_pin_mut().poll_shutdown(cx)
    }
}
