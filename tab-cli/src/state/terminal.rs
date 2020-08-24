#[derive(Clone, Debug)]
pub struct TerminalSizeState(pub (u16, u16));

impl Default for TerminalSizeState {
    fn default() -> Self {
        let dimensions = crossterm::terminal::size().expect("failed to get terminal size");

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

impl TerminalMode {
    pub fn is_echo(&self) -> bool {
        match self {
            TerminalMode::Echo => true,
            _ => false,
        }
    }
}
