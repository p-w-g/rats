# rats

Monorepo for `rat` and its npm distribution wrapper, `rat-nest`.

`rat` is a small, personal CLI for running the same shell command across
every subdirectory of a workspace, in parallel or in sequence, instead of
cd-ing into each one by hand. It's a Rust rewrite of an earlier C# tool
(`ath`).

## What's here

- **[`rat/`](rat/)** — the CLI itself, written in Rust. `rat fep <command>`
  fans a command out across every immediate subdirectory of the working
  folder; `--recursive` walks the whole subtree instead, for
  monorepo-of-monorepos layouts. Skip/only filters, a persisted default
  folder, timeouts, and concurrency control round it out. See
  [`rat/README.md`](rat/README.md) for the full CLI reference.
- **[`rat-nest/`](rat-nest/)** — the `@p-w-g/ratnest` npm package. It has no
  CLI logic of its own; it downloads, checksum-verifies, and execs the
  prebuilt `rat` binary for your platform, so `npm install -g
  @p-w-g/ratnest` gives you the `rat` command without a Rust toolchain. See
  [`rat-nest/README.md`](rat-nest/README.md).

## Why one repo

`rat/` is the source of truth for behavior; `rat-nest/` only repackages its
releases for npm. Keeping both in one repo keeps that packaging atomic:
pushing a `vX.Y.Z` tag on the `release` branch builds `rat`'s binaries for
every supported platform, stamps `X.Y.Z` into both `rat/Cargo.toml` and
`rat-nest/package.json`, cuts a GitHub Release, and — after a manual
approval click — runs `npm publish`. There's no separate version bump to
remember in a second repo. See
[`.github/workflows/release.yml`](.github/workflows/release.yml).

## This is personal tooling

`rat` exists to save its author from terminal-jockeying across repos and
packages by hand. It's deliberately small and dependency-light so it stays
portable - the goal is for `rat`, this repo, and its release pipeline to
keep working wherever their author's day-to-day work goes next, not to stay
pinned to one employer's toolchain.

## Development

Each project builds and tests independently:

```bash
cd rat && cargo build && cargo test
cd rat-nest && npm install && npm test
```

CI ([`.github/workflows/rust.yml`](.github/workflows/rust.yml)) runs `rat`'s
test suite on every push; release automation lives in
[`.github/workflows/release.yml`](.github/workflows/release.yml).
