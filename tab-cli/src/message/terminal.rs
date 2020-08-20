#[derive(Debug)]
pub enum TerminalSend {
    Stdin(Vec<u8>),
}

#[derive(Debug)]
pub enum TerminalRecv {
    Stdout(Vec<u8>),
}
