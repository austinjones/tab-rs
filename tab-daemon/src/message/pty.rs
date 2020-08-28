use super::tab::TabInput;
use crate::state::pty::PtyScrollback;

use tab_api::{chunk::OutputChunk, tab::TabMetadata};

/// Terminates the PTY connection & supporting services.
#[derive(Debug, Clone)]
pub struct PtyShutdown {}

/// Requests to the PTY connection from the Daemon.
/// Messaged on the `PtyBus`, and carried from the `ListenerPtyCarrier`.
///
/// Usage:
/// - Tx from `PtyService`, to communicate initialization to the daemon and to provide events for CLI connections.
/// - Rx from `PtyScrollbackService`, to collect scrollback in the daemon process.
#[derive(Debug, Clone)]
pub enum PtyRecv {
    Init(TabMetadata),
    Scrollback,
    /// Resizes to the given number of (cols, rows)
    Resize((u16, u16)),
    Input(TabInput),
    Terminate,
}

/// Events generated in the PTY process, forwarded to the Daemon.
/// Messaged on the `PtyBus`, and carried to the `ListenerBus` over the `ListenerPtyCarrier`.
///
/// Usage:
/// - Tx from `PtyService`, to forward events from the websocket
/// - Rx from `PtyScrollbackService`, to listen for Scrollback requests.
/// - Rx from `ListenerPtyCarrier`, to forward events to the daemon & CLI connections.
#[derive(Debug, Clone)]
pub enum PtySend {
    Started(TabMetadata),
    Output(OutputChunk),
    Scrollback(PtyScrollback),
    Stopped,
}
