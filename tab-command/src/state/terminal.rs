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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalMode {
    /// Terminal is in raw mode, capturing stdin, and forwarding raw stdout
    Echo,
    /// Terminal is in interactive mode, using Crossterm.
    Crossterm,
}

impl Default for TerminalMode {
    fn default() -> Self {
        Self::Crossterm
    }
}
