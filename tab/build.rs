extern crate clap;

use clap::Shell;
use std::{env, path::PathBuf};

include!("src/cli.rs");

fn main() {
    let mut app = app();

    let mut path = std::env::current_dir().expect("failed to get current dir");
    path.push("target");
    path.push("completions");
    std::fs::create_dir_all(path.as_path()).expect("failed to create ./target/completions");

    for shell in [
        Shell::Bash,
        Shell::Elvish,
        Shell::Fish,
        Shell::PowerShell,
        Shell::Zsh,
    ]
    .iter()
    {
        app.gen_completions("tab", shell.clone(), path.as_path());
    }
}
