use clap::{App, Arg, SubCommand};

fn main() {
    println!("Hello, world!");
}

fn init() {
    let matches = App::new("Terminal Multiplexer")
        .version("0.1")
        .author("Austin Jones <implAustin@gmail.com>")
        .about("Provides persistent terminal sessions with multiplexing.")
        .arg(
            Arg::with_name("TAB")
                .help("Switches to the provided tab")
                .required(true)
                .index(1),
        )
        .subcommand(
            SubCommand::with_name("list")
                .about("lists open terminal sessions")
                .version("0.1")
                .arg(
                    Arg::with_name("debug")
                        .short("d")
                        .help("print debug information verbosely"),
                ),
        )
        .get_matches();
}
