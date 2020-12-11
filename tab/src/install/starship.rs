use std::{fs::Permissions, os::unix::prelude::PermissionsExt, path::PathBuf};

use toml_edit::{table, value, Array, Document};

use super::{Package, PackageBuilder, PackageEnv};

pub fn starship_package(env: &PackageEnv) -> Package {
    let mut package = PackageBuilder::new("starship");

    let config = starship_toml(env);
    package.edit(
        config,
        Permissions::from_mode(0o644),
        edit,
        "add [custom.tab] section which invokes `tab --starship`",
    );

    package.build()
}

fn starship_toml(env: &PackageEnv) -> PathBuf {
    if let Ok(path) = std::env::var("STARSHIP_CONFIG") {
        return path.into();
    }

    let mut path = env.home.clone();
    path.push(".config");
    path.push("starship.toml");
    path
}

fn edit(string: Option<String>) -> String {
    let string = string.unwrap_or("".to_string());
    let toml = string.parse::<Document>();

    if let Err(e) = toml {
        eprintln!(
            "Failed to parse `starship.toml` as a TOML document: {}",
            e.to_string()
        );
        return string;
    }

    let mut toml = toml.unwrap();

    if toml["custom"].is_none() {
        toml["custom"] = table();
    }

    if toml["custom"]["tab"].is_none() {
        toml["custom"]["tab"] = table();
    }

    toml["custom"]["tab"] = table();
    toml["custom"]["tab"]["description"] = value("The current tab in the tab terminal multiplexer");
    toml["custom"]["tab"]["command"] = value("tab --starship");
    toml["custom"]["tab"]["when"] = value("tab --starship");

    let mut array = Array::default();
    array.push("sh").ok();
    toml["custom"]["tab"]["shell"] = value(array);

    toml["custom"]["tab"]["format"] = value("[$output]($style) ");
    toml["custom"]["tab"]["style"] = value("bold blue");
    toml["custom"]["tab"].as_inline_table();

    toml.to_string_in_original_order()
}
