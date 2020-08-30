//! `tab`, a modern terminial multiplexer designed for overwhelmed software & systems engineers.
//!

pub mod cli;
use cli::init;

pub fn main() -> anyhow::Result<()> {
    let args = init();

    // create the dotdir path, so the modules don't need to worry about it.
    tab_api::config::mkdir()?;

    if let Some(launch) = args.value_of("LAUNCH") {
        match launch {
            "daemon" => tab_daemon::daemon_main(),
            "pty" => tab_pty::pty_main(),
            _ => panic!("unsupported --_launch value"),
        }
    } else if let Some(completion_script) = args.value_of("COMPLETION") {
        match completion_script {
            "bash" => print!("{}", include_str!("completions/bash/tab.bash")),
            "elvish" => print!("{}", include_str!("completions/elvish/tab.elv")),
            "fish" => print!("{}", include_str!("completions/fish/tab.fish")),
            "powershell" => print!("{}", include_str!("completions/powershell/_tab.ps1")),
            "zsh" => print!("{}", include_str!("completions/zsh/_tab")),
            _ => panic!("unsupported completion script: {}", completion_script),
        };

        Ok(())
    } else {
        tab_command::command_main(args)
    }
}
