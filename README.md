# $
quick app to open sh in the current directory with a privacy respecting prompt

## usage
```
dollar [shell]
```
ran with no args, it runs your `$SHELL`, falling back to `sh` if you use one that isn't supported.
you can pass a shell name or path to use a specific one like this:
```
dollar bash
dollar /bin/zsh
dollar ksh
etc...
```
supports sh, dash, bash, ksh, zsh and fish (for now!).

you can pass `-h` or `--help` to print usage
