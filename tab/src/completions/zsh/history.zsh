if [ -n "$TAB" ] ; then
    # check that tab is installed.
    tab -V 2>&1 > /dev/null

    if [ $? -ne 0 ]; then
        return;
    fi

    # try to retrieve the histfile location, this time echoing stderr if there is an issue
    HIST=$(tab --_histfile zsh "$TAB")
    if [ $? -ne 0 ]; then
        return;
    fi

    # export the HISTFILE override
    export HISTFILE="$HIST"
fi