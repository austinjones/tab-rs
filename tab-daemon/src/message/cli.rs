use super::tab::TabScrollback;

use tab_api::{
    chunk::{InputChunk, OutputChunk},
    tab::{CreateTabMetadata, TabId, TabMetadata},
};

/// The CLI connection Send message.  Messaged on the CliBus, and
///
/// Generally used to Subscribe, Unsubscribe, and otherwise manage tabs.
///
/// Usage:
/// - Tx from CliService, to create, delete, edit, and retask tabs.
/// - Tx from CliService, to send stdin
/// - Rx into ListenerConnectionCarrier
#[derive(Debug, Clone)]
pub enum CliSend {
    /// Provides a stdin chunk for the given tab
    Input(TabId, InputChunk),
    /// Creates a tab with the given metadata.  Ignored if a tab with the given name is already active.
    CreateTab(CreateTabMetadata),
    /// Requests that any clients who are subscribed to the given tab be retasked, to the second tab
    Retask(TabId, TabId),
    /// Requests the scrollback buffer be read, and replied to as a CliRecv::Scrollback message.
    RequestScrollback(TabId),
    /// Resizes the tab to the given number of (cols, rows)
    ResizeTab(TabId, (u16, u16)),
    /// Closes the tab with the given ID
    CloseTab(TabId),
    /// Closes the tab with the given name, if one exists.
    CloseNamedTab(String),
    /// Shuts down the Daemon and all PTY processes
    GlobalShutdown,
}

/// The CLI connection Recv message.  
/// The main point of contact for a CLI connection.  
/// Messaged on the CliBus.
///
/// Used to receive tab lifecycle events, and stdout.
///
/// Usage:
/// - Tx from ListenerConnectionCarrier, for subscribed tabs which have been subscribed to by the CLI connection.
/// - Rx into CliService, to be forwarded on the websocket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliRecv {
    /// A notification that a tab with the given metadata has started, and is ready for subscriptions.
    TabStarted(TabMetadata),
    /// A notification that scrollback is available for the given tab.
    /// Receivers can clone the scrollback buffer with TabScrollback::scrollback
    Scrollback(TabScrollback),
    /// A notification that a tab has been retasked.  The client may need to request scrollback and change their subscriptions.
    Retask(TabId, TabId),
    /// A notification that a tab has been terminated.
    TabStopped(TabId),
    /// An indexed stdout chunk, for the given tab
    Output(TabId, OutputChunk),
}

/// Terminates the websocket connection & supporing services.
/// The daemon is not affected.  If you want to stop the daemon,
/// use CliSend::GlobalShutdown
#[derive(Debug, Clone)]
pub struct CliShutdown {}
