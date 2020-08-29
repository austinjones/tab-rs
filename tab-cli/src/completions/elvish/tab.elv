# `tab` autocompletions for elvish

edit:completion:arg-completer[tab] = [@words]{
    fn spaces [n]{
        repeat $n ' ' | joins ''
    }
    fn cand [text desc]{
        edit:complex-candidate $text &display-suffix=' '(spaces (- 14 (wcswidth $text)))$desc
    }
    command = 'tab'
    for word $words[1:-1] {
        if (has-prefix $word '-') {
            break
        }
        command = $command';'$word
    }
    completions = [
        &'tab'= {
            cand --_launch 'launches the daemon or a new pty process with `tab --_launch [daemon|pty]'
            cand -w 'closes the tab with the given name'
            cand --close 'closes the tab with the given name'
            cand -l 'lists the active tabs'
            cand --list 'lists the active tabs'
            cand -W 'terminates the tab daemon and all active pty sessions'
            cand --shutdown 'terminates the tab daemon and all active pty sessions'
            cand -h 'Prints help information'
            cand --help 'Prints help information'
            cand -V 'Prints version information'
            cand --version 'Prints version information'
        }
    ]
    $completions[$command]
}
