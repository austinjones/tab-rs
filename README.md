# tab

**tab, an intuitive, config-driven terminal multiplexer designed for software & systems engineers**

- **Configuration-driven:**  `tab` provides persistent, configurable tabs which you can rely on to organize your daily context-switches.
- **Intuitive and shell-oriented:**.  Tabs are listed, selected, and closed with a single command, `tab`.  Tab has _one_ disconnect escape sequence, `ctrl-W`.  Tab uses your terminal emulator's natural scrollback buffer.  Tab has first-class support for `bash`, `fish`, and `zsh`.
- **State-agnostic:**  You can ask `tab` to do anything, from anywhere.  Tabs are selected & closed using the same interface, regardless of whether they are attached, running, terminated, etc.
- **Rich & dynamic auto-complete:**  Your library of tabs are auto-completed when switching to a new tab with `tab <TAB>` .  Your running tabs are auto-completed when closing a tab with `tab -w <TAB>`.
- **Low-latency & Efficient:**  Tab is written in Rust, and has an async message-based architecture to minimize latency.  Tab has a round-trip latency (stdin to stdout) of ~5ms.  The tab daemon uses 0.2% when idle, 1-2% CPU during normal usage, and 5% when a tab is throwing extreme amounts of stdout.

## Quickstart
Quick installation & usage instructions:
```
$ cargo install tab
$ tab foo/     > to open a tab.
$ tab bar/     > to switch to another tab.  
                works within an active session!
$ echo $TAB   > to view the active tab.  
                put this in your promptline, 
                or get https://starship.rs/
$ tab -w foo  > to close a tab.
$ tab -l      > to view the tabs
$ ctrl-W      > to disconnect the session
```

Tab adds trailing slashes to tab names.  This makes autocomplete between `foo/` and it's children, `foo/bar/` work nicely.  You can type `tab foo` and tab will add the slashes for you.

# Installation
Tab currently supports `MacOS` and `Linux`.  Tab supports the `bash`, `fish`, and `zsh` shells.

## 1. Install the binary

Using Homebrew: 
```
brew install austinjones/taps/tab
```

Using cargo: 
```
cargo install tab
```

Or, from prebuilt-binaries: 
[https://github.com/austinjones/tab-rs/releases/latest](https://github.com/austinjones/tab-rs/releases/latest)

## 2. Install autocompletions for your shell

Tab works best when configured with shell autocompletions.  Tab will install completions for you:
- `tab --install all` - will install completions for all supported shells which are installed on your system
- `tab --install bash` - installs completions for bash
- `tab --install fish` - installs completions for fish
- `tab --install zsh` - installs completions for zsh

## 3. Configure your statusline

**(Starship)**

Tab integrates with the [starship](https://starship.rs/) prompt, and can auto-configure the integration:
- `tab install --starship`

Additionally, if you want, you can customize your prompt order in `~/.config/starship.toml`.  This is what I have in my configuration.  It's cleaner than the default order:
```
prompt_order = ["custom.tab", "directory", "git_branch", "cmd_duration", "line_break", "character"]
```

**(Starship Alternatives)**

You can configure any other statusline tool that supports environment variables.  The current tab name is available in the `$TAB` environment var.

You can also add a handcrafted statusline snippet to your shell's rc configuration file, in
[bash](https://github.com/austinjones/tab-rs/blob/master/tab/src/completions/bash/statusline.bash), 
[fish](https://github.com/austinjones/tab-rs/blob/master/tab/src/completions/fish/statusline.fish),
or [zsh](https://github.com/austinjones/tab-rs/blob/master/tab/src/completions/zsh/statusline.zsh).



# Configuration
Tab supports persistent `tab.yml` configurations.  There are two types of configurations:
- Workspace configurations, which are active within any subdirectory, and link to repositories.
- Repository configurations, which define tab endpoints.  Your typical `tab` interaction would be switching
  to one of these repositories via `tab myproj/`

A full set of example files are available in the [examples](https://github.com/austinjones/tab-rs/tree/master/examples) directory, but here are some starters:

```
~/workspace/tab.yml:

workspace:
  - repo: my-project/
  - tab: workspace-tab
    doc: "this is a top-level workspace tab"
```


```
~/workspace/my-project/tab.yml:

repo: proj
doc: "my project"

tabs:
  - tab: run
    dir: src/
    doc: "runs the project server"
```

With these configurations, `tab -l` provides the following:
```
$ tab -l
Available tabs:
    proj/             (my project)
    proj/run/         (runs the project server)
    workspace-tab/    (this is a top-level workspace tab)
```

# Security
Tab can execute commands in a terminal, so I take security seriously.  This is how I protect your machine in `tab`:

The `tab` daemon requires the following to accept any websocket connection:
- The request must include a 128 byte auth token, stored in the file: `~/.tab/daemon-pid.yml`.  On unix operating systems, the file is assigned the permissions `600`.
- The `Origin` header must not be present in the request.  This prevents any connection from a browser.
- The daemon binds to `127.0.0.1` on a random port.  This should prevent any attempted connections from the local network.