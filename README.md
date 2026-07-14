# fzd

[![Crates.io](https://img.shields.io/crates/v/fzd?logo=rust&logoColor=white)](https://crates.io/crates/fzd)
[![npm](https://img.shields.io/npm/v/@ervan0707/fzd?logo=npm)](https://www.npmjs.com/package/@ervan0707/fzd)
[![PyPI](https://img.shields.io/pypi/v/fzd?logo=pypi&logoColor=white)](https://pypi.org/project/fzd/)
[![CI](https://img.shields.io/github/actions/workflow/status/ervan0707/fzd/ci.yml?branch=main&label=CI&logo=github)](https://github.com/ervan0707/fzd/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](#license)

Interactive terminal directory explorer that `cd`'s your shell.

`fzd` opens a fuzzy, keyboard-driven TUI for browsing directories. It draws its
UI to stderr and prints only the chosen absolute path to stdout, so a
tiny shell wrapper can capture that path and run the real `cd` in your current
shell.

## Install

Pick whichever ecosystem you already live in — every channel ships the same
binary.

| Channel | Command |
|---------|---------|
| Cargo | `cargo install fzd` |
| npm | `npm install -g @ervan0707/fzd` |
| PyPI | `pip install fzd` |
| Script | `curl -fsSL https://raw.githubusercontent.com/ervan0707/fzd/main/install.sh \| bash` |
| Nix | `nix profile install github:ervan0707/fzd` |

## Shell integration

Because a child process can't change its parent shell's directory, `fzd` prints
the picked path and a wrapper does the `cd`. For fish, source the bundled
function:

```fish
source /path/to/shell/fzd.fish
```

Then:

```fish
fzd            # browse from the current directory
fzd ~/code     # browse starting somewhere else
fzd --jump     # jump straight to frecent / bookmarked dirs
fzd -j         # short form
```

## Usage

```
fzd [OPTIONS] [PATH]

Arguments:
  [PATH]  Directory to start in (defaults to the current directory)

Options:
  -j, --jump        Start directly in jump mode (frecent + bookmarked dirs)
  -a, --all         Show hidden (dot) files from the start
      --print-only  Do not record the accepted directory in the frecency store
      --check-update  Check GitHub for a newer release, then exit
      --update        Download the latest GitHub release and replace this binary
      --force         With --update, replace a package-manager-managed binary
  -h, --help        Print help
  -V, --version     Print version
```

## Updating

`fzd --check-update` compares your installed version against the latest GitHub
release and prints whether you're current, without opening the TUI.

`fzd --update` goes further: it downloads the release binary for your platform
and swaps it in over the running executable. This is meant for the raw-binary
install (the `curl | bash` script or a manual download). If fzd was installed
through a package manager, `--update` stops and points you at the right command
(`cargo install fzd --force`, `npm update -g @ervan0707/fzd`, `brew upgrade fzd`, and so
on). Pass `--force` to self-replace anyway.

## Development

This repo ships a Nix flake and a pinned Rust toolchain:

```sh
nix develop          # enter the dev shell (or `direnv allow` with the .envrc)
cargo run            # run locally
cargo build --release
```

## Releasing

Releases are fully automated from [Conventional Commits](https://www.conventionalcommits.org/)
via `semantic-release` in GitHub Actions. A `feat:`/`fix:`/`feat!:` commit on
`main` computes the next version, builds binaries for six platforms, and
publishes to crates.io, npm, PyPI, and GitHub Releases in one run. See
`.github/workflows/release.yml`.

## License

MIT
