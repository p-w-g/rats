# rat

`rat` is a small command-line tool for running the same shell command across
every subdirectory of a workspace, in parallel. Point it at a folder full of
repos or packages and it fans a command out to each of them at once (with
optional skip/only filters, timeouts, and a persisted default working
directory), instead of you cd-ing into each one by hand.

It's a Rust rewrite of an earlier C# tool (`ath`); this repo is the current,
actively developed version.

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) 1.85+ (the project uses the
  2024 edition). Installing Rust via [rustup](https://rustup.rs/) also gives
  you `cargo`, which is all you need below.

## Build from source

```bash
git clone https://github.com/p-w-g/rat.git
cd rat
cargo build --release
```

The compiled binary is written to `target/release/rat` (`rat.exe` on
Windows).

## Install

The simplest option is to let Cargo build and install it into `~/.cargo/bin`
(already on your `PATH` if you installed Rust via rustup):

```bash
cargo install --path .
```

Then, from any terminal:

```bash
rat help
```

Prefer to manage the binary yourself instead? Copy it wherever you keep
personal tools and make sure that location is on your `PATH`:

```bash
# after `cargo build --release`
cp target/release/rat /usr/local/bin/        # macOS/Linux example
```

On Windows, copy `target\release\rat.exe` to a folder that's on your `PATH`
(or add `target\release` to `PATH` directly).

## Usage

`rat help` always prints the current, in-the-box copy of everything below.
There are three top-level commands:

```
rat help            # this help text
rat fep <command>   # run <command> in every subfolder of the working folder
rat pipe <steps...> # run an ordered, readiness-gated pipeline of different commands
rat cfg <subcommand> # read/edit rat's own config
```

### `fep`: run a command across every subfolder

```
rat fep <command...> [flags]
```

`<command...>` is whatever you want run, exactly as you'd type it on the
command line - e.g. `rat fep git pull`, `rat fep npm install`. It's run
through your platform's shell (`cmd.exe` on Windows, `/bin/bash` elsewhere)
once per immediate subdirectory of the working folder, in parallel.

`[flags]` are **rat's own** flags, not your command's. All optional:

| flag                  | meaning                                                                                          |
| --------------------- | ------------------------------------------------------------------------------------------------- |
| `--local`              | use the current directory for this run, even if a default folder is set via `cfg here`            |
| `--recursive` / `--r`  | walk the whole subtree, not just immediate subfolders - see below                                  |
| `--concurrency-4`      | run at most 4 directories at once (default: number of CPUs)                                        |
| `--sync`               | run exactly one directory at a time (equivalent to `--concurrency-1`); wins over `--concurrency` if both are given |
| `--only-uk-fi`         | only run in subfolders with a `-`-separated name component containing `uk` or `fi` (also accepts `--only-uk,fi`) - see below |
| `--skip-priv-corp`     | skip subfolders with a name component containing `priv` or `corp` (combines with `--only`, see below) |
| `--sustain`            | wait as long as it takes, ignoring any timeout                                                     |
| `--timeout-30`         | timeout *this run* after 30 seconds, overriding the configured timeout                             |

> `--timeout-0` is a **0-second timeout**, not "disabled" - that's different
> from `cfg to 0`, which does disable it. Use `--sustain` or `cfg nto` if you
> want no timeout.

### `--sync`: one directory at a time

By default `fep` fans a command out to every subfolder at once. Use `--sync`
when that's not safe for your workflow - e.g. "for every repo: checkout
master, create a feature branch, make changes, commit, push" is the kind of
multi-step, stateful sequence you generally want to run one repository at a
time rather than in parallel. `--sync` runs exactly one directory at a time;
it's equivalent to `--concurrency-1`, just with clearer intent at the call
site (and it wins if you also pass `--concurrency`).

### `--recursive`/`--r`: monorepo-of-monorepos layouts

By default `fep` only reaches the *immediate* subfolders of the working
folder. If your workspace folder contains `foo/` and `bar/`, and each of
those is itself a workspace of packages (`foo/baz/`, `bar/qux/`), plain
`fep` run from the workspace root never reaches `foo/baz/` or `bar/qux/` -
only `foo/` and `bar/` themselves. Add `--recursive` (or its short alias
`--r`) to walk the whole subtree instead, running the command in every
matching directory at every depth:

```bash
rat fep --recursive git status
```

A directory excluded by `cfg ignore` or `--skip` is pruned from the walk,
not just excluded from the run - `fep` never descends into it in the first
place. This matters for anything dependency-tree-shaped: without it,
`rat fep --recursive rm -rf node_modules` would crawl every package inside
every `node_modules` it finds before ever getting a chance to delete one.
Pair `--recursive` with `--skip-node_modules` (or a standing `cfg ignore
node_modules`) to prune those trees instead:

```bash
rat fep --recursive --skip-node_modules rm -rf node_modules
```

### `--only`/`--skip`: component-aware directory matching

A subfolder's name is split into components on `-` (e.g. `uk-priv-app`
tokenizes to `uk`, `priv`, `app`); `--only`/`--skip` match a component by
**substring**, not an arbitrary substring of the folder's *path*. `--only-uk`
matches any component containing `uk` - so besides `uk-priv-app` (component
`uk`), it also matches folders like `ukpr-app` and `nlukx-app` (components
`ukpr` and `nlukx`, with no dash isolating `uk`), not just a component
that's exactly `uk`. This is deliberately looser than a `uk-pr-app`-style
naming scheme requires: if you pack multiple codes into one component with
no delimiter (e.g. `ukpr-app` = `uk` + `pr`), a filter for either half -
`--only-uk` or `--only-pr` - still finds it, wherever in the component it
sits. Given

```
uk-priv-app  uk-corp-app  fi-priv-app  fi-corp-app  nl-priv-app  at-corp-app
```

| flag                        | selects                                    |
| ---------------------------- | ------------------------------------------- |
| `--only-uk`                  | `uk-priv-app`, `uk-corp-app`                |
| `--only-app`                 | all six (`app` is a component of every one) |
| `--only-corp`                | every corporate app                         |
| `--skip-priv`                | everything except the private apps          |
| `--only-uk --skip-corp`      | `uk-priv-app`                               |
| `--only-app --skip-fi`       | every app except the Finnish ones           |

`--only` and `--skip` **both apply when given**: a folder must satisfy
`--only` (if present) *and* not match `--skip` (if present). A single flag
can also list multiple components as comma- or dash-separated values (OR
within that flag): `--only-uk,fi` and `--only-uk-fi` both mean "UK or FI".

> Earlier versions matched by substring against a subfolder's full path (so
> a filter word appearing in a parent folder could match every subfolder),
> and `--skip` was ignored entirely whenever `--only` was also given. Both
> of those have changed to the component-aware, combining behavior above.

### The #1 gotcha: your command's flags vs. rat's flags

rat parses `--flags` out of the command you pass to `fep` *before* your
command ever runs - your shell doesn't get a say. Specifically:

- Any `--word...` in your command where `word` is one of rat's reserved
  words (`local`, `skip`, `only`, `sustain`, `timeout`, `sync`, `recursive`,
  `r`) is captured by rat as its own flag and **never reaches your
  command**.
- Any other unrecognized `--flag` is **silently dropped** - not passed to
  your command, not treated as a rat option, just gone.
- Single-dash flags (`-m`, `-rf`, `-n`, ...) are **never** touched by rat and
  always reach your command untouched.

| you type                                        | what happens                                                                 |
| ------------------------------------------------ | ----------------------------------------------------------------------------- |
| `rat fep rm -rf node_modules`                     | safe - `-rf` is single-dash, passed straight through                          |
| `rat fep git commit -m "this fails with ath"`     | safe - `-m` is single-dash                                                    |
| `rat fep git merge --skip-commit`                 | **not safe** - rat reads `--skip-commit` as its own `--skip commit` filter; git runs as plain `git merge`, silently missing `--skip-commit` |

If you need to run something with a `--flag` that collides with rat's
reserved words, put the real invocation in a small script and call the
script instead (`rat fep ./do-the-thing.sh`), so nothing reaches rat's
parser except the script name.

### `pipe`: ordered, readiness-gated process pipelines

`fep` fans **one** command out across **many** directories. `pipe` is the
opposite shape: **different** commands, in **different** directories, run
in declared order - where a step can be backgrounded (a dev server, a
build-then-serve) and the next step shouldn't start until it's actually
ready, not just until some fixed delay has passed.

```
rat pipe [step-flags] -- <command...> [--then [step-flags] -- <command...> ...]
```

Everything before a step's own `--` is that step's flags; everything after
it, up to the next `--then` or the end of the command line, is the literal
command to run for that step.

| step flag               | meaning                                                                 |
| ------------------------ | -------------------------------------------------------------------------|
| `--dir <path>`           | directory to run this step in (default: current directory)              |
| `--bg`                   | background this step instead of waiting for it to exit; requires a readiness rule |
| `--ready-port <n>`       | wait until something accepts a TCP connection on `127.0.0.1:<n>`        |
| `--ready-match <text>`   | wait until a line of this step's stdout/stderr contains `<text>`        |
| `--ready-timeout <secs>` | how long to wait for readiness before giving up (default: 30)           |

Example - start Sanity Studio's dev server, wait until it's actually
listening, then start a blog dev server that depends on it, without two
terminal tabs:

```bash
rat pipe --dir studio --bg --ready-port 3333 -- npm run dev \
  --then --dir blog -- npm run dev
```

Both steps stream their output live, prefixed with the step's label (the
last path component of `--dir`). Once every declared step has started, rat
supervises any still-running background steps until Ctrl-C or one of them
exits unexpectedly - tearing all of them down, whole process tree included,
either way. A failing step (or a background step that never becomes ready)
tears down everything already started and stops the pipeline immediately,
rather than letting later steps run against a foundation that isn't there.

`pipe` has its own small grammar and does **not** share `fep`'s
`--flag-value` parsing or its silent-drop of unrecognized flags - a typo'd
step flag is a hard error here.

### `cfg`: configuration

Config lives at `~/.ratconfig` (JSON), created on first use of `cfg`.

| subcommand                          | meaning                                                                 |
| ------------------------------------ | ------------------------------------------------------------------------ |
| `cfg path`                           | print the config file's path                                            |
| `cfg file`                           | print the config file's contents                                        |
| `cfg here`                           | set the current directory as the default working folder for `fep`      |
| `cfg away`                           | unset the default working folder (back to using CWD)                    |
| `cfg ignore <folders...>`            | permanently ignore these folders in every `fep` run, e.g. `cfg ignore .git .idea` |
| `cfg heed <folders...>` / `--all`    | stop ignoring these folders, or clear the whole ignore list with `--all` |
| `cfg to <seconds>`                   | set a default timeout for `fep`; `0` disables it                        |
| `cfg nto`                            | disable the default timeout                                             |

> `.git` is always excluded from `fep` runs, even with an empty `cfg
> ignore` list - it's not a preference to override, just something nobody
> wants a fanned-out command run inside. Everything else you want skipped
> (`node_modules`, `.idea`, ...) is up to `cfg ignore`.

## Development

```bash
cargo test      # run the test suite
cargo build     # debug build
```

## Roadmap

- Packaged release binaries (GitHub Releases) for macOS/Linux/Windows.
- An npm wrapper package so JS/TS developers can `npm install -g` it without
  a local Rust toolchain.
