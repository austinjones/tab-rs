//! `tab`, a modern terminial multiplexer designed for overwhelmed software & systems engineers.
//!

pub mod cli;
mod install;

use anyhow::Context;
use cli::init;
use tab_api::{config::history_path, log::set_level, tab::normalize_name};

const TAB_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn main() -> anyhow::Result<()> {
    let args = init();

    // create the dotdir path, so the modules don't need to worry about it.
    tab_api::config::mkdir()?;
    std::env::set_var("TAB_BIN", std::env::current_exe()?);

    // log level should always be available due to the clap default
    // however, if the parse fails, we should return an error and alert the user.
    if let Some(level) = args.value_of("LOG") {
        let level = level
            .parse()
            .context("failed to parse the value provided to --log <level>")?;
        set_level(level);
    }

    if let Some(launch) = args.value_of("LAUNCH") {
        match launch {
            "daemon" => tab_daemon::daemon_main(TAB_VERSION),
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
    } else if let Some(shell) = args.value_of("HISTFILE-SHELL") {
        let tab = args.value_of("TAB-NAME").ok_or_else(|| {
            anyhow::format_err!("a tab name is required for the --_histfile command")
        })?;

        if shell == "fish" {
            return Err(anyhow::format_err!(
                "fish does not use a historyfile, and instead uses the fish_history env var"
            ));
        }

        let histfile = history_path(shell, &normalize_name(tab))?;
        print!("{}", histfile.to_string_lossy());

        Ok(())
    } else if let Some(install_args) = args.values_of("INSTALL") {
        install::run(install_args, args.is_present("YES"))
    } else if args.is_present("STARSHIP") {
        // used for the starship prompt
        let tab = std::env::var("TAB");

        if tab.is_err() {
            std::process::exit(1);
        }

        print!("tab {}", tab.unwrap());

        Ok(())
    } else {
        let exit_code = tab_command::command_main(args, TAB_VERSION)?;
        std::process::exit(exit_code);
    }
}
