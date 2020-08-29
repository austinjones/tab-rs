use clap::App;
use clap::Arg;
use clap::ArgMatches;

pub fn init() -> ArgMatches<'static> {
    app().get_matches()
}

fn app() -> App<'static, 'static> {
    App::new("Terminal Multiplexer")
        .version("v0.1")
        .name("tab")
        .author("Austin Jones <implAustin@gmail.com>")
        .about("`tab`, a modern terminal multiplexer designed for overwhelmed software & systems engineers.")
        .long_about(include_str!("help/about.txt"))
        .after_help(include_str!("help/after-help.txt"))
        .arg(
            Arg::with_name("LAUNCH")
                .long("_launch")
                .required(false)
                .hidden(true)
                .takes_value(true)
                .possible_values(&["daemon", "pty"])
                .help("launches the daemon or a new pty process with `tab --_launch [daemon|pty]"),
        )
        .arg(
            Arg::with_name("AUTOCOMPLETE-TAB")
                .long("_autocomplete_tab")
                .hidden(true)
                .takes_value(false)
                .help("autocompletes for the `tab <SELECT>` command"),
        )
        .arg(
            Arg::with_name("AUTOCOMPLETE-CLOSE-TAB")
                .long("_autocomplete_close_tab")
                .hidden(true)
                .takes_value(false)
                .help("autocompletes for the `tab -w <CLOSE>` command"),
        )
        .arg(
            Arg::with_name("LIST")
                .short("l")
                .long("list")
                .display_order(0)
                .help("lists the active tabs"),
        )
        .arg(
            Arg::with_name("SHUTDOWN")
                .short("W")
                .long("shutdown")
                .takes_value(false)
                .display_order(2)
                .help("terminates the tab daemon and all active pty sessions"),
        )
        .arg(
            Arg::with_name("CLOSE-TAB")
                .short("w")
                .long("close")
                .takes_value(true)
                .value_name("TAB")
                .help("closes the tab with the given name")
        )
        .arg(
            Arg::with_name("COMPLETION")
                .long("completion")
                .required(false)
                .takes_value(true)
                .possible_values(&["bash", "elvish", "fish", "powershell", "zsh"])
                .help("prints the raw tab completion script"),
        )
        .arg(
            Arg::with_name("TAB-NAME")
                .help("switches to the provided tab")
                .required(false)
                .value_name("TAB")
                .default_value("any/")
                .conflicts_with_all(&["CLOSE-TAB", "LIST", "SHUTDOWN"])
                .index(1),
        )
}
