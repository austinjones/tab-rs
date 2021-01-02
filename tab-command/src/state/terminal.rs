use tab_api::tab::TabId;

use crate::env::terminal_size;

/// The client's view of the current terminal size
#[derive(Clone, Debug)]
pub struct TerminalSizeState(pub (u16, u16));

impl Default for TerminalSizeState {
    fn default() -> Self {
        let dimensions = terminal_size().expect("failed to get terminal size");

        TerminalSizeState(dimensions)
    }
}

/// The current terminal mode.
/// This type is used to guarantee consistency of the terminal state.
/// When the terminal mode is changed, the terminal state is fully reset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalMode {
    /// No terminal program is active
    None,
    /// Terminal is in raw mode, capturing stdin, and forwarding raw stdout (for the given tab name)
    Echo(TabId),
    /// Terminal is in interactive finder mode, using Crossterm.
    FuzzyFinder,
}

impl Default for TerminalMode {
    fn default() -> Self {
        Self::None
    }
}
