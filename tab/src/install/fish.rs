use std::{fs::Permissions, os::unix::prelude::PermissionsExt, path::PathBuf};

use super::{Package, PackageBuilder, PackageEnv};

pub fn fish_package(env: &PackageEnv) -> Package {
    let mut package = PackageBuilder::new("fish");

    package.write_file(
        completion_path(env),
        include_str!("../completions/fish/tab.fish"),
        "an autocompletion script for the fish shell",
        Permissions::from_mode(0o755),
    );

    package.build()
}

fn completion_path(env: &PackageEnv) -> PathBuf {
    let mut path = env.home.clone();

    path.push(".config");
    path.push("fish");
    path.push("completions");
    path.push("tab.fish");

    path
}
