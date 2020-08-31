
# `tab` autocompletions for PowerShell

using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'tab' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'tab'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-')) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'tab' {
            [CompletionResult]::new('--_launch', '_launch', [CompletionResultType]::ParameterName, 'launches the daemon or a new pty process with `tab --_launch [daemon|pty]')
            [CompletionResult]::new('-w', 'w', [CompletionResultType]::ParameterName, 'closes the tab with the given name')
            [CompletionResult]::new('--close', 'close', [CompletionResultType]::ParameterName, 'closes the tab with the given name')
            [CompletionResult]::new('-l', 'l', [CompletionResultType]::ParameterName, 'lists the active tabs')
            [CompletionResult]::new('--list', 'list', [CompletionResultType]::ParameterName, 'lists the active tabs')
            [CompletionResult]::new('-W', 'W', [CompletionResultType]::ParameterName, 'terminates the tab daemon and all active pty sessions')
            [CompletionResult]::new('--shutdown', 'shutdown', [CompletionResultType]::ParameterName, 'terminates the tab daemon and all active pty sessions')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Prints help information')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Prints help information')
            [CompletionResult]::new('-V', 'V', [CompletionResultType]::ParameterName, 'Prints version information')
            [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Prints version information')
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
