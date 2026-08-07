# Shell completion

`ctddump completions <shell>` prints a completion script to standard output. Once
your shell loads that script, typing `ctddump ` and pressing Tab suggests the
subcommands, and Tab-completing a partial word (a subcommand, a flag, or an enum
value such as a shell name) fills it in when only one candidate remains.

Supported shells: `bash`, `zsh`, `fish`, `elvish`, `powershell`.

The command only writes the script; where you put it is what makes completion
permanent. Pick the section for your shell.

## Bash

```bash
# Try it in the current shell
source <(ctddump completions bash)

# Install for every new shell (user-local)
mkdir -p ~/.local/share/bash-completion/completions
ctddump completions bash > ~/.local/share/bash-completion/completions/ctddump
```

This relies on the `bash-completion` package being installed and sourced from
your `~/.bashrc`. Without it, add `source <(ctddump completions bash)` to
`~/.bashrc` directly.

## Zsh

```zsh
# A directory on your $fpath, created if needed
mkdir -p ~/.zsh/completions
ctddump completions zsh > ~/.zsh/completions/_ctddump
```

Make sure that directory is on `$fpath` and completion is initialised, before the
`compinit` call in your `~/.zshrc`:

```zsh
fpath=(~/.zsh/completions $fpath)
autoload -Uz compinit && compinit
```

Start a new shell (or run `compinit`) to pick up the change.

## Fish

```fish
mkdir -p ~/.config/fish/completions
ctddump completions fish > ~/.config/fish/completions/ctddump.fish
```

Fish loads it automatically in new shells.

## PowerShell

```powershell
# Add to your profile so it loads every session
ctddump completions powershell | Out-String | Invoke-Expression
```

To make it permanent, append that line to the file at `$PROFILE`.

## Elvish

```elvish
ctddump completions elvish > ~/.config/elvish/lib/ctddump.elv
# then, in rc.elv:
use ctddump
```

## Regenerating after an upgrade

The script encodes the commands and flags of the version that produced it. After
upgrading `ctddump`, regenerate it with the same command so new subcommands and
options complete correctly.
