# this is a statusline snippet for the zsh shell
# installation: 
# - append the snippet to ~/.zshrc

setopt prompt_subst
if (($+TAB)); then
  PROMPT="%{$fg[green]%}tab ${TAB}%{$reset_color%} $PROMPT"
fi