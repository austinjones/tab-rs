# `tab`, a modern terminal multiplexer designed for overwhelmed software & systems engineers
Tab is a terminal multiplexer written in `Rust`.

## Features:
- Tab is _configuration-driven_.  `tab` provides persistent tab suggestions which always initialize in the correct working directory.
  Configuration is defined in simple `tab.yml` files placed in your workspace and repository roots.
- Tab is _shell-oriented_, and _minimialistic_.  Tabs are listed, selected, and closed with a single command, `tab`.  Tab has _one_ disconnect escape sequence, `ctrl-W`.  Tab provides tab-unique command history.
- Tab provides _rich autocomplete_.  Your library of tabs are completed when switching to a new tab with `tab <TAB>` .  Your running tabs are completed when closing a tab with `tab -w <TAB>`.
- Tab is _state-agnostic_.  You can invoke `tab` to do anything, from anywhere.  If a tab isn't running, it's started.  If a tab is running, it's reconnected (and possibly shared with another client).  If you have a tab selected, and you close it with `tab -w mytab`, your session is disconnected.
- Tab is _low-latency_, and _efficient_.  It has a round-trip latency (stdin to stdout) of ~5ms.  The tab daemon uses 0.2% when idle, 1-2% CPU during normal usage, and 5% when a tab is throwing extreme amounts of stdout.

## Quickstart
Quick installation & usage instructions:
```
$ cargo install --git https://github.com/austinjones/tab-rs.git
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

## Configuration
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

# Shell Configuration
Tab works best when you configure your terminal with autocomplete, and a statusbar.

## Starship
The best way to configure the command prompt is to use [starship](https://starship.rs/).

In `~/.config/starship.toml`, add:
```toml
[env_var]
variable = "TAB"
prefix = "tab "
style = "bold green"
```

## Bash
`tab` supports dynamic autocompletion and a custom statusbar in bash.

1. Install the the autocompletion script:
```
mkdir -p ~/.tab && tab --completion bash > ~/.tab/_tab.bash
```

2. Source the completion script from `~/.bashrc`
```bash
source ~/.tab/_tab.bash
```

3. If you want to add a custom statusline, add to ~/.bashrc
```
PS1="tab ${TAB:-/} $ "
```

## Fish
`tab` supports dynamic autocompletion and a custom statusbar in fish.

1. Install the autocompletion script to your fish completions directory.
```
mkdir -p ~/.config/fish/completions && tab --completion fish > ~/.config/fish/completions/tab.fish
```

2. If you want to use a custom command prompt, you can add to `~/.config/fish/config.fish`
```bash
function fish_prompt
  if test -n "$TAB"
    set_color $fish_color_cwd
    printf 'tab %s' "$TAB" 
    set_color normal
    printf ' in '
    set_color $fish_color_cwd
    printf '%s' (basename $PWD)
    set_color normal
    echo " \$ "
  else
    set_color $fish_color_cwd
    printf '%s' (basename $PWD)
    set_color normal
    echo " \$ "
  end
end
```

## ZSH
`tab` supports dynamic autocompletion and a custom statusbar in zsh.

1. Install OhMyZsh, and copy the TODO LINK `completions/zsh/_tab` script to `${ZSH_CUSTOM}/plugins/tab/_tab`.
```
mkdir -p "${ZSH_CUSTOM}/plugins/tab/" && tab --completion > "${ZSH_CUSTOM}/plugins/tab/_tab"
```

2. Add `tab` to your `plugins` list in `~/.zshrc`:
```zsh
# load the `tab` autocompletions
plugins=(git tab)
autoload -U compinit && compinit

source $ZSH/oh-my-zsh.sh
```

3. If you want to use a custom prompt, add to `~/.zshrc`
```zsh
# add the selected tab to the prompt
setopt prompt_subst
if (($+TAB)); then
  PROMPT="%{$fg[green]%}tab ${TAB}%{$reset_color%} $PROMPT"
fi
```

# Security
Tab can execute commands in a terminal, so I take security seriously.  This is how I protect your machine in `tab`:


The `tab` daemon requires the following to accept any websocket connection:
- The request must include a 128 byte auth token, stored in the file: `~/.tab/daemon-pid.yml`.  On unix operating systems, the file is assigned the permissions `600`.
- The `Origin` header must not be present in the request.  This prevents any connection from a browser.
- Websocket listeners bind to `127.0.0.1` on a random port.  This should prevent any attempted connections from the local network.