# `tab` autocompletions for fish

complete -c tab -n "__fish_use_subcommand" -f -a '(tab --_autocomplete_tab)' 
complete -c tab -n "__fish_use_subcommand" -s w -l close -d 'closes the tab with the given name' -x -a '(tab --_autocomplete_close_tab)'

complete -c tab -n "__fish_use_subcommand" -s l -l list -d 'lists the active tabs'
complete -c tab -n "__fish_use_subcommand" -s W -l shutdown -d 'terminates the tab daemon and all active pty sessions'
complete -c tab -n "__fish_use_subcommand" -s h -l help -d 'Prints help information'
complete -c tab -n "__fish_use_subcommand" -s V -l version -d 'Prints version information'