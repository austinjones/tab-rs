use crossterm::terminal;
use tab_api::env::is_raw_mode;

pub fn terminal_size() -> anyhow::Result<(u16, u16)> {
    if is_raw_mode() {
        terminal::size().map_err(|err| err.into())
    } else {
        Ok((80, 24))
    }
}
