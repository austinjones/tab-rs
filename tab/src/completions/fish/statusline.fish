# this is a statusline snippet for the fish shell
# installation: 
# - append the snippet to ~/.config/fish/config.fish

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