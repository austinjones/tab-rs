#[derive(Debug, Clone)]
pub enum TerminalSend {
    Stdin(Vec<u8>),
}

#[derive(Debug, Clone)]
pub enum TerminalRecv {
    Stdout(Vec<u8>),
}
