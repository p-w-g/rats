# @p-w-g/ratnest

npm distribution wrapper for the [`rat`](../rat) CLI, both living in the
[`rats`](https://github.com/p-w-g/rats) monorepo. This package contains **no
CLI logic of its own** — it exists purely to let people run:

```sh
npm install -g @p-w-g/ratnest
rat fep ...
rat --help
```

The Rust project in `../rat` remains the source of truth for behavior. This
package only locates, downloads, verifies, caches, and executes the prebuilt
`rat` binary for the user's platform. The `ratnest` name is never exposed as a
command — only `rat` is (see the `bin` field in `package.json`).

## How it works

1. On `npm install`, `postinstall` best-effort predownloads the binary for
   the current platform (see below for why this is allowed to fail).
2. Running `rat` invokes [`bin/rat.js`](bin/rat.js), which ensures the binary
   is present (downloading and checksum-verifying it on demand if the
   postinstall step didn't run or failed), then execs it with the original
   argv, stdio, and exit code.

## Module layout

Each concern lives in its own file under [`src/`](src):

| File | Responsibility |
| --- | --- |
| [`platform.js`](src/platform.js) | Validates `process.platform`/`process.arch` against the platforms rat ships. |
| [`releaseUrl.js`](src/releaseUrl.js) | Builds the GitHub Release asset URL and SHA256SUMS URL for a given version + platform. |
| [`download.js`](src/download.js) | Fetches a URL to a local file (atomic: writes to `.part`, then renames), and verifies a file's SHA-256 against SHA256SUMS. |
| [`cache.js`](src/cache.js) | Orchestrates the above: resolves the OS cache dir, checks it, downloads + verifies + extracts if missing. |
| [`exec.js`](src/exec.js) | Spawns the binary, forwarding stdio, signals, and exit code. |
| [`errors.js`](src/errors.js) | Friendly, actionable error messages for the failure modes users actually hit. |

[`bin/rat.js`](bin/rat.js) wires `cache` + `exec` together and is the only
file referenced by `package.json`'s `bin` field. [`scripts/postinstall.js`](scripts/postinstall.js)
calls the same `ensureBinary()` eagerly at install time.

## Versioning

**This package's `version` in `package.json` is the single source of truth**
for which Rust release to fetch. `cache.js` reads its own version and
downloads from the GitHub Release tagged `v<version>` — it does not query
GitHub for "latest". This is enforced automatically, not by hand: pushing a
`vX.Y.Z` tag on the `release` branch runs the whole pipeline (see
[`rats/.github/workflows/release.yml`](../.github/workflows/release.yml)) —
build the Rust binaries, stamp `X.Y.Z` into both `rat/Cargo.toml` and this
package's `package.json`, create the GitHub Release, then (after a manual
approval click) `npm publish`. There's no manual two-repo version bump left
to forget.

## Release asset naming contract

`releaseUrl.js` expects each GitHub Release (tag `vX.Y.Z`) to contain one
archive per supported platform, named:

```
rat-<platform>-<arch>.tar.gz   # linux, darwin
rat-<platform>-<arch>.zip      # win32
```

e.g. `rat-linux-x64.tar.gz`, `rat-darwin-arm64.tar.gz`, `rat-win32-x64.zip` —
`<platform>`/`<arch>` are Node's own `process.platform`/`process.arch`
values, not Rust target triples. No version in the filename; it's implied by
the release tag. Each archive contains a single file, `rat` (`rat.exe` on
win32), at its root. The release also includes one `SHA256SUMS` file
covering every archive — `cache.js` downloads and verifies against it before
trusting anything.

## Supported platforms / adding a new one

Currently: linux (x64, arm64), macOS (x64, arm64), Windows (x64).

To add a platform, add one entry to `SUPPORTED` in
[`src/platform.js`](src/platform.js) and make sure the release workflow
builds and uploads the matching `rat-<platform>-<arch>` asset. No other file
needs to change.

## Why (almost) no dependencies

- Downloading uses Node's built-in `https` module (with manual redirect
  following for GitHub's release-asset redirect) instead of an HTTP client
  dependency. An earlier version used the global `fetch`, but on Node 24 /
  Windows, calling `process.exit()` *anywhere later in the same process*
  after any `fetch()` call crashes with a libuv assertion
  (`UV_HANDLE_CLOSING`, `src/win/async.c`) — reproduced independently of this
  package's logic. Since `exec.js` must call `process.exit()` to forward the
  wrapped CLI's real exit code, and that would happen on essentially every
  fresh Windows install (first run always downloads), `fetch` was not safe
  to use here. `https.get` has no such issue.
- Checksum verification uses Node's built-in `crypto` module — no hashing
  dependency needed.
- Archive extraction shells out to the system `tar` (present by default on
  macOS, Linux, and Windows 10 1803+ — its bundled `bsdtar` auto-detects zip
  as well as gzip) instead of adding an archive-extraction npm package. This
  is the one place a dependency-free approach costs something: it trusts an
  OS-provided tool being on `PATH` rather than a vendored implementation.
- The binary cache lives in the OS-standard per-user cache directory
  (`XDG_CACHE_HOME`/`~/.cache` on Linux, `~/Library/Caches` on macOS,
  `%LOCALAPPDATA%` on Windows), resolved by a few lines in `cache.js` rather
  than a cache-directory-resolution dependency.

## Why `postinstall` failures don't fail `npm install`

`postinstall` predownloads the binary so the first `rat` invocation is fast,
but it swallows its own errors and exits 0. Environments that install
offline, behind a restrictive proxy, or with `--ignore-scripts` would
otherwise have a broken `npm install -g` entirely. `bin/rat.js` calls the
same `ensureBinary()` lazily, so nothing is lost — worst case, the first
`rat` invocation pays the download cost instead of `npm install`.
