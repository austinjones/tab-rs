use crate::env::terminal_size;

#[derive(Clone, Debug)]
pub struct TerminalSizeState(pub (u16, u16));

impl Default for TerminalSizeState {
    fn default() -> Self {
        let dimensions = terminal_size().expect("failed to get terminal size");

        TerminalSizeState(dimensions)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalMode {
    Echo,
    Crossterm,
}

impl Default for TerminalMode {
    fn default() -> Self {
        Self::Crossterm
    }
}
