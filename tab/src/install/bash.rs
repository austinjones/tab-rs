use std::path::PathBuf;

use super::{Package, PackageBuilder, PackageEnv, ScriptAction, Shell};

pub fn bash_package(env: &PackageEnv) -> Package {
    let mut package = PackageBuilder::new("bash");

    let completion = completion_script_path(env);
    package.write_file(
        completion.clone(),
        include_str!("../completions/bash/tab.bash"),
        "an autocompletion script for the bash shell",
    );

    package
        .script(Shell::Bash, bashrc(env), "source the tab.bash script")
        .action(ScriptAction::SourceFile(completion))
        .build();

    package.build()
}

fn completion_script_path(env: &PackageEnv) -> PathBuf {
    let mut path = env.data.clone();
    path.push("tab");
    path.push("completion");
    path.push("tab.bash");
    path
}

fn bashrc(env: &PackageEnv) -> PathBuf {
    let mut path = env.home.clone();
    path.push(".bashrc");

    path
}
