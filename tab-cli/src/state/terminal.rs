#[derive(Clone, Debug)]
pub struct TerminalSizeState(pub (u16, u16));

impl Default for TerminalSizeState {
    fn default() -> Self {
        let dimensions = crossterm::terminal::size().expect("failed to get terminal size");

        TerminalSizeState(dimensions)
    }
}
