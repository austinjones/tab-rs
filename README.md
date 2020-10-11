# tab

**The intuitive, config-driven terminal multiplexer designed for software & systems engineers**

<img align="right" width=400 height=400 src="./readme/tab-vectr.svg">

**- Intuitive:**  Tabs are discovered, selected, and closed with a single command, `tab`, and _one_ escape sequence, `ctrl-W`.

**- Config-driven:**  `tab` provides persistent, configurable tabs with unique command history, working directories, and docstrings.

**- State-agnostic:**  Tab provides a simple, consistent interface that works anywhere, in any state.

**- Autocompleted:**  Tab provides dynamic autocomplete, so you can get oriented, and context switch fast.

**- Fast:**  Tabs launch in 50ms, and reconnect in 10ms.  Keyboard latency (stdin to stdout) is under 5ms.

# Quickstart
Quick installation & usage instructions:
```
$ brew install austinjones/taps/tab
  OR
$ cargo install tab
  THEN
$ tab --install all 
  # installs shell autocompletion scripts and statusline integrations

$ tab foo/     # to open a tab.
$ tab bar/     # to switch to another tab.  
                 works within an active session!
$ echo $TAB    # to view the active tab.  
                 put this in your promptline, 
                 or get https://starship.rs/
$ tab -w baz   # to close a tab (or many tabs).
$ tab -l       # to view the tabs
$ ctrl-W       # to disconnect the session
```

Tab adds trailing slashes to tab names.  This improves autocomplete between tabs and subtabs (e.g. `tab/` and `tab/child/`).

# Installation
Tab currently supports `MacOS` and `Linux`.  Tab supports the `bash`, `fish`, and `zsh` shells.

## 1. Install the binary

The `tab` binary can be installed using Homebrew, Cargo, or from binaries.

**(Homebrew)**
```
brew install austinjones/taps/tab
```

**(Cargo)**
```
cargo install tab
```

**(Binaries)**

Download binaries from:
[https://github.com/austinjones/tab-rs/releases/latest](https://github.com/austinjones/tab-rs/releases/latest)

**(Known Issues)**

After you upgrade tab or move the tab binary, you may want to run the `tab --shutdown` command to restart the daemon.  See [#163](https://github.com/austinjones/tab-rs/issues/163).

If you get the message `tab: unsupported terminal app`, you fix it by removing the `osx` plugin from your `~/.zshrc`.  See [#156](https://github.com/austinjones/tab-rs/issues/156).

## 2. Install autocompletions for your shell
Tab works best when configured with shell autocompletions.  

Tab has a built-in script installer which provides a detailed explanation of the changes, and prompts for your confirmation.

**(All)**

Tab can install completions for all shells & supported integration which are present on your system.
```
tab --install all
```

**(Bash | Fish | Zsh)**

Tab can also install completions for a specific shell.
```
tab --install bash
tab --install fish
tab --install zsh
```


## 3. Configure your statusline

**(Starship)**

Tab integrates with the [starship](https://starship.rs/) prompt, and can auto-configure the integration:

```
tab --install starship
```

You can also put the current tab before the directory in `~/.config/starship.toml`.  This is how I've configured my own shell:
```
format = "${custom.tab}$all"
```

**(Other)**

You can configure any other statusline tool that supports environment variables.  The current tab name is available in the `$TAB` environment var.

You can also add a handcrafted statusline snippet to your shell's rc configuration file, in
[bash](https://github.com/austinjones/tab-rs/blob/main/tab/src/completions/bash/statusline.bash), 
[fish](https://github.com/austinjones/tab-rs/blob/main/tab/src/completions/fish/statusline.fish),
or [zsh](https://github.com/austinjones/tab-rs/blob/main/tab/src/completions/zsh/statusline.zsh).

# Navigation
Tab is designed to provide quick navigation between tabs, and persistent navigation to workspaces or repositories.  In these examples, the prefix before the `$` is the selected tab.

To select a tab:
```
$ tab foo
foo/ $
```

To switch to another tab while within a session, and drop back to the root shell:
```
foo/ $ tab bar
bar/ $ exit
$ 
```

To switch to another tab while within an interactive application:
```
monitor/ $ top
... top output ...
[ctrl-W]
$ tab foo
foo/ $ 
```

Each workspace has it's own tab.  You can use this to quickly reset the working directory within a workspace:
```
repo/ $ tab workspace
workspace/ $
```

To switch to another workspace (if a workspace link has been configured in the current workspace [tab.yml](https://github.com/austinjones/tab-rs/blob/main/examples/advanced-workspace/tab.yml)):
```
workspace/ $ tab other-workspace
other-workspace $
```

# Configuration
Tab supports persistent `tab.yml` configurations.  There are two types of configurations:
- Workspace configurations, which are active within any subdirectory, and link to repositories.
- Repository configurations, which define tab endpoints.  Your typical `tab` interaction would be switching to one of these repositories via `tab myproj/`.

Detailed documentation is available in the [examples](https://github.com/austinjones/tab-rs/tree/main/examples/) directory, but here are some starter configurations:

```
~/workspace/tab.yml:

workspace:
  - repo: my-project/
  - tab: workspace-tab
    doc: "this is a top-level workspace tab"
  - workspace: ../other-workspace
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
    other-workspace/  (workspace tab for ~/other-workspace)
```

# Security
Tab can execute commands in a terminal, so I take security seriously.  This is how I protect your machine in `tab`:

The `tab` daemon requires the following to accept any websocket connection:
- The request must include a 128 byte auth token, stored in the file: `~/.tab/daemon-pid.yml`.  On unix operating systems, the file is assigned the permissions `600`.
- The `Origin` header must not be present in the request.  This prevents any connection from a browser.
- The daemon binds to `127.0.0.1`.  This should prevent any attempted connections from the local network.
