//! `tab`, a modern terminial multiplexer designed for overwhelmed software & systems engineers.
//!
use clap::App;
use clap::Arg;
use clap::ArgMatches;

pub fn main() -> anyhow::Result<()> {
    let args = init();

    if let Some(launch) = args.value_of("LAUNCH") {
        match launch {
            "daemon" => tab_daemon::daemon_main(),
            "pty" => tab_pty::pty_main(),
            _ => panic!("unsupported --_launch value"),
        }
    } else {
        tab_cli::cli_main(args)
    }
}

fn init() -> ArgMatches<'static> {
    App::new("Terminal Multiplexer")
        .version("0.1")
        .name("tab")
        .author("Austin Jones <implAustin@gmail.com>")
        .about("`tab`, a modern terminal multiplexer designed for overwhelmed software & systems engineers.")
        .long_about(include_str!("about.txt"))
        .after_help(include_str!("after-help.txt"))
        // .arg(
        //     Arg::with_name("DEBUG")
        //         .long("debug")
        //         .required(false)
        //         .takes_value(false)
        //         .help("enables debug logging"),
        // )
        .arg(
            Arg::with_name("LAUNCH")
                .long("_launch")
                .required(false)
                .takes_value(true)
                .hidden(true),
        )
        .arg(
            Arg::with_name("AUTOCOMPLETE-TAB")
                .long("_autocomplete_tab")
                .hidden(true)
                .takes_value(false)
                .help("runs the daemon using `cargo run`"),
        )
        .arg(
            Arg::with_name("AUTOCOMPLETE-CLOSE-TAB")
                .long("_autocomplete_close_tab")
                .hidden(true)
                .takes_value(false)
                .help("runs the daemon using `cargo run`"),
        )
        .arg(
            Arg::with_name("CLOSE-TAB")
                .short("w")
                .takes_value(true)
                .help("closes the tab with the given name"),
        )
        .arg(
            Arg::with_name("SHUTDOWN")
                .short("W")
                .long("shutdown")
                .takes_value(false)
                .help("terminates the tab daemon and all active pty sessions"),
        )
        .arg(
            Arg::with_name("LIST")
                .short("l")
                .long("list")
                .help("lists the active tabs"),
        )
        .arg(
            Arg::with_name("TAB-NAME")
                .help("switches to the provided tab")
                .required(false)
                .index(1),
        )
        .get_matches()
}
