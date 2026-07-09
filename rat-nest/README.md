# @p-w-g/ratnest

npm distribution wrapper for the [`rat`](../rat) CLI. This package contains **no CLI
logic of its own** — it exists purely to let people run:

```sh
npm install -g @p-w-g/ratnest
rat fep ...
rat --help
```

The Rust project in `../rat` remains the source of truth for behavior. This
package only locates, downloads, caches, and executes the prebuilt `rat`
binary for the user's platform. The `ratnest` name is never exposed as a
command — only `rat` is (see the `bin` field in `package.json`).

## How it works

1. On `npm install`, `postinstall` best-effort predownloads the binary for
   the current platform (see below for why this is allowed to fail).
2. Running `rat` invokes [`bin/rat.js`](bin/rat.js), which ensures the binary
   is present (downloading it on demand if the postinstall step didn't run or
   failed), then execs it with the original argv, stdio, and exit code.

## Module layout

Each concern lives in its own file under [`src/`](src):

| File | Responsibility |
| --- | --- |
| [`platform.js`](src/platform.js) | Maps `process.platform`/`process.arch` to a Rust target triple. |
| [`releaseAsset.js`](src/releaseAsset.js) | Builds the GitHub Release asset URL for a given version + platform. |
| [`download.js`](src/download.js) | Fetches a URL to a local file (atomic: writes to `.part`, then renames). |
| [`installer.js`](src/installer.js) | Orchestrates the above: checks the local cache, downloads if missing, chmods. |
| [`run.js`](src/run.js) | Spawns the binary, forwarding stdio, signals, and exit code. |
| [`errors.js`](src/errors.js) | Friendly, actionable error messages for the failure modes users actually hit. |

[`bin/rat.js`](bin/rat.js) wires `installer` + `run` together and is the only
file referenced by `package.json`'s `bin` field. [`scripts/postinstall.js`](scripts/postinstall.js)
calls the same `ensureBinary()` eagerly at install time.

## Versioning

**This package's `version` in `package.json` is the single source of truth**
for which Rust release to fetch. `installer.js` reads its own version and
downloads from the GitHub Release tagged `v<version>` — it does not query
GitHub for "latest". This means:

- Publishing a new npm version *is* the act of pointing at a new Rust release.
- The two version numbers must always be bumped together: cut the `rat`
  release first (tag `vX.Y.Z`), then set `rat-nest/package.json`'s `version`
  to `X.Y.Z` and publish.

## Release asset naming contract

`releaseAsset.js` expects each GitHub Release (tag `vX.Y.Z`) to contain one
binary asset per supported platform, named:

```
rat-<target-triple>       # unix
rat-<target-triple>.exe   # windows
```

e.g. `rat-x86_64-unknown-linux-gnu`, `rat-aarch64-apple-darwin`,
`rat-x86_64-pc-windows-msvc.exe`. This is a plain, uncompressed binary
upload — no `.tar.gz`/`.zip`, so the npm side never needs an archive
extraction dependency. The Rust project does not yet have a release workflow
that produces these; one needs to be added under `rat/.github/workflows/`
building/uploading exactly these filenames before this wrapper can download
anything real.

## Supported platforms / adding a new one

Currently: linux (x64, arm64), macOS (x64, arm64), Windows (x64).

To add a platform, add one entry to `PLATFORM_MAP` in
[`src/platform.js`](src/platform.js) and make sure the release workflow
builds and uploads the matching `rat-<target-triple>` asset. No other file
needs to change.

## Why no dependencies

- Downloading uses Node's built-in `https` module (with manual redirect
  following for GitHub's release-asset redirect) instead of an HTTP client
  dependency. An earlier version used the global `fetch`, but on Node 24 /
  Windows, calling `process.exit()` *anywhere later in the same process*
  after any `fetch()` call crashes with a libuv assertion
  (`UV_HANDLE_CLOSING`, `src/win/async.c`) — reproduced independently of this
  package's logic. Since `run.js` must call `process.exit()` to forward the
  wrapped CLI's real exit code, and that would happen on essentially every
  fresh Windows install (first run always downloads), `fetch` was not safe
  to use here. `https.get` has no such issue.
- There's no archive extraction dependency because release assets are raw
  binaries, not archives.
- The binary cache lives inside this package's own install directory
  (`.bin-cache/`, gitignored) rather than a shared OS cache dir, so there's
  no need for a cache-directory-resolution dependency either — npm already
  owns this directory's lifecycle (created on install, removed on uninstall
  or reinstall).

## Why `postinstall` failures don't fail `npm install`

`postinstall` predownloads the binary so the first `rat` invocation is fast,
but it swallows its own errors and exits 0. Environments that install
offline, behind a restrictive proxy, or with `--ignore-scripts` would
otherwise have a broken `npm install -g` entirely. `bin/rat.js` calls the
same `ensureBinary()` lazily, so nothing is lost — worst case, the first
`rat` invocation pays the download cost instead of `npm install`.
