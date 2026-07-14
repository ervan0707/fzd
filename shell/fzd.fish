# fzd — fish shell integration
#
# `fzd` (the binary) draws its UI to stderr and prints only the chosen
# directory to stdout. This wrapper captures that path and performs the real
# `cd`, since a child process can't change its parent shell's directory.
#
# Install:
#   1. Build the binary and put it on PATH, e.g.
#        nix build && cp result/bin/fzd ~/.local/bin/
#      (or `cargo build --release && cp target/release/fzd ~/.local/bin/`)
#   2. Source this file from your config, e.g. add to ~/.config/fish/config.fish:
#        source /path/to/dirs/shell/fzd.fish
#      or copy it into ~/.config/fish/functions/fzd.fish
#
# Usage:
#   fzd            # browse from the current directory
#   fzd ~/code     # browse starting somewhere else
#   fzd --jump     # jump straight to frecent/bookmarked dirs
#   fzd -j         # short form

function fzd --description 'Interactively pick a directory and cd into it (fzd)'
    set -l target (command fzd $argv)
    if test -n "$target"; and test -d "$target"
        cd "$target"
    end
end
