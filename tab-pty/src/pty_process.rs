use log::{error, info};
use std::{
    collections::HashMap,
    process::{Command, ExitStatus},
    sync::Arc,
};
use tab_api::chunk::{InputChunk, OutputChunk};
use tab_pty_process::CommandExt;
use tab_pty_process::{
    AsyncPtyMaster, AsyncPtyMasterReadHalf, AsyncPtyMasterWriteHalf, Child, PtyMaster,
};
use time::Duration;
use tokio::sync::broadcast::RecvError;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{
        broadcast::{Receiver, Sender},
        mpsc::error::SendError,
    },
    time,
};

// ! TODO: move into tab-pty-process

static CHUNK_LEN: usize = 2048;
static OUTPUT_CHANNEL_SIZE: usize = 32;
static STDIN_CHANNEL_SIZE: usize = 32;
