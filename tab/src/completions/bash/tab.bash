# `tab` autocompletions for bash

_tab_select() {
    tab --_autocomplete_tab
}

_tab_close() {
    tab --_autocomplete_close_tab
}

_tab() {
    local cur prev opts
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    case "$prev" in
    -w)
        TABS=$(tab --_autocomplete_close_tab)
        COMPREPLY=( $(compgen -W "${TABS}" -- $cur) )
        return 0
        ;;
    --close)
        TABS=$(tab --_autocomplete_close_tab)
        COMPREPLY=( $(compgen -W "${TABS}" -- $cur) )
        return 0
        ;;
    --completion)
        COMPREPLY=( $(compgen -W "bash elvish fish powershell zsh") )
        return 0
        ;;
    esac

    case "$cur" in
    --completion=)
        TABS=$(tab --_autocomplete_close_tab)
        COMPREPLY=( $(compgen -W "bash elvish fish powershell zsh") )
        return 0
        ;;
    -*)
        opts=" -h --help -l --list -w --close -W --shutdown -V --version --completion <TAB> "
        COMPREPLY=( $(compgen -W "${opts}") )
        return 0
        ;;
    *)
        TABS=$(tab --_autocomplete_tab)
        COMPREPLY=( $(compgen -W "${TABS}" -- $cur) )
        return 0
        ;;
    esac
}

complete -F _tab tab
