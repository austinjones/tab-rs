#[derive(Debug)]
pub struct MainShutdown {}

#[derive(Debug)]
pub enum MainRecv {
    SelectTab(String),
    SelectInteractive,
}
