use lifeline::impl_storage_clone;
use std::{collections::HashMap, path::PathBuf, process::ExitStatus};
use tab_api::chunk::{InputChunk, OutputChunk};

/// Terminates the process, websocket connection, and via cancellation the connected PTY shell session
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainShutdown {}

/// Terminates the PTY process.  Forwarded to the `MainBus` as a `MainShutdown`.
#[derive(Debug, Clone)]
pub struct PtyShutdown {}

/// A request sent to the PTY process.
///
/// Carried over the `PtyBus`.
///
/// Usage:
/// - Rx into the `PtyService`, to write input into the shell.
/// - Tx from the `ClientSessionService`, to forward websocket requests from the daemon.
#[derive(Debug, Clone)]
pub enum PtyRequest {
    Resize((u16, u16)),
    Input(InputChunk),
    Shutdown,
}

/// A response sent by the PTY shell manager to the PtyBus.
///
/// Carried over the `PtyBus`
///
/// Usage:
/// - Rx into the `ClientSessionService`, to forward messages along the websocket to the daemon.
/// - Tx from the `PtyService`, to forward stdout and termination messages.
#[derive(Debug, Clone)]
pub enum PtyResponse {
    Output(OutputChunk),
    Terminated(ExitStatus),
}

/// Describes options which can be set for the launched shell process
#[derive(Debug, Clone)]
pub struct PtyOptions {
    /// The shell dimensions in (columns, rows)
    pub dimensions: (u16, u16),
    /// The shell command
    pub command: String,
    /// Arguments to pass to the shell command
    pub args: Vec<String>,
    /// The working directory of the launched process
    pub working_directory: PathBuf,
    /// Environment variables to set for the launched process.
    pub env: HashMap<String, String>,
}

impl_storage_clone!(PtyOptions);
