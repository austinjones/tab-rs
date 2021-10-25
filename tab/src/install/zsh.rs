use std::{fs::Permissions, os::unix::prelude::PermissionsExt, path::PathBuf, process::Command};

use anyhow::bail;

use super::{Package, PackageBuilder, PackageEnv, ScriptAction, Shell};

pub fn zsh_package(env: &PackageEnv) -> anyhow::Result<Package> {
    let mut package = PackageBuilder::new("zsh");

    // Install completions
    let completion_ohmyzsh = completion_ohmyzsh(env);
    let completion_usr_local = completion_usr_local_share();
    let tab_completions = include_str!("../completions/zsh/_tab");

    if let Some(ohmyzsh) = &completion_ohmyzsh {
        package.clean_file(ohmyzsh.clone());
    }

    if let Some(usrlocal) = &completion_usr_local {
        package.clean_file(usrlocal.clone());
    }

    if let Some(ohmyzsh) = completion_ohmyzsh {
        package.write_file(
            ohmyzsh,
            tab_completions,
            "the tab completion script for the oh-my-zsh package manager",
            Permissions::from_mode(0o755),
        );
    } else if let Some(usrlocal) = completion_usr_local {
        package.write_file(
            usrlocal,
            tab_completions,
            "the tab completion script",
            Permissions::from_mode(0o755),
        );
    } else {
        bail!("tab could not find a suitable completion directory.  supported directories are: '~/.oh-my-zsh/completions' and '/usr/local/share/zsh/site-functions'");
    }

    // Install history
    let zshrc = zshrc(env);
    let history_script = history_script(env);
    package.write_file(
        history_script.clone(),
        include_str!("../completions/zsh/history.zsh"),
        "script which sets a tab-unique $HISTFILE when within a session",
        Permissions::from_mode(0o755),
    );

    package
        .script(
            Shell::Zsh,
            zshrc,
            Permissions::from_mode(0o644),
            "source the history.zsh script",
        )
        .action(ScriptAction::SourceFile(history_script))
        .build();

    package.postinstall_action(run_compinit, "Remove ~/.zcompdump and execute compinit");

    Ok(package.build())
}

fn run_compinit() -> anyhow::Result<()> {
    println!("Installing zsh... running compinit...");

    Command::new("zsh")
        .args(&["-i", "-c", "rm ~/.zcompdump*; compinit"])
        .spawn()?
        .wait()?;

    Ok(())
}

fn history_script(env: &PackageEnv) -> PathBuf {
    let mut path = env.data.clone();
    path.push("tab");
    path.push("completion");
    path.push("zsh-history.zsh");

    path
}

fn zshrc(env: &PackageEnv) -> PathBuf {
    let mut path = env.home.clone();
    path.push(".zshrc");

    path
}

fn completion_ohmyzsh(env: &PackageEnv) -> Option<PathBuf> {
    let mut path = env.home.clone();

    path.push(".oh-my-zsh");
    if !path.exists() {
        return None;
    }

    path.push("completions");
    path.push("_tab");

    Some(path)
}

fn completion_usr_local_share() -> Option<PathBuf> {
    let mut path = PathBuf::from("/");
    path.push("usr");
    path.push("local");
    path.push("share");
    path.push("zsh");
    path.push("site-functions");

    if !path.exists() {
        return None;
    }

    path.push("_tab");
    Some(path)
}
