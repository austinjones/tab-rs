# `tab` autocompletions for fish

complete -c tab  -f -a '(tab --_autocomplete_tab)' 
# hack here with `-o w`, to get fish to insert a space after the `tab -w` completion
complete -c tab -n "__fish_use_subcommand" -o w -l close -d 'closes the tab with the given name' -x -a '(tab --_autocomplete_close_tab)'
complete -c tab -n "__fish_use_subcommand" -o z -l disconnect -d 'disconnects any active sessions for the given tab' -x -a '(tab --_autocomplete_close_tab)'

complete -c tab -l completion -d 'prints raw autocomplete scripts' -x -a 'bash elvish fish powershell zsh'
complete -c tab -n "__fish_use_subcommand" -s k -l check -d 'checks the current workspace for errors and warnings'
complete -c tab -n "__fish_use_subcommand" -s l -l list -d 'lists the active tabs'
complete -c tab -n "__fish_use_subcommand" -s W -l shutdown -d 'terminates the tab daemon and all active pty sessions'
complete -c tab -n "__fish_use_subcommand" -s h -l help -d 'Prints help information'
complete -c tab -n "__fish_use_subcommand" -s V -l version -d 'Prints version information'

