use crate::state::pty::PtyScrollback;

use tab_api::{
    chunk::{InputChunk, OutputChunk},
    tab::TabMetadata,
};

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
    Input(InputChunk),
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
    /// The pty has been resized to the given number of (cols, rows)
    Resized((u16, u16)),
    Stopped,
}

impl PartialEq for PtySend {
    fn eq(&self, other: &Self) -> bool {
        match self {
            PtySend::Started(tab) => {
                if let PtySend::Started(other_tab) = other {
                    return tab == other_tab;
                } else {
                    return false;
                }
            }
            PtySend::Output(output) => {
                if let PtySend::Output(other_output) = other {
                    return output == other_output;
                } else {
                    return false;
                }
            }
            PtySend::Scrollback(_scrollback) => {
                // we can't implement this, as scrollback contains an async mutex
                return false;
            }
            PtySend::Stopped => {
                if let PtySend::Stopped = other {
                    return true;
                } else {
                    return false;
                }
            }
            PtySend::Resized(size) => {
                if let PtySend::Resized(other) = other {
                    return size == other;
                } else {
                    return false;
                }
            }
        }
    }
}
