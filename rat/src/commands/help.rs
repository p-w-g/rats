pub fn show_help() {
    println!("{}", help_text());
}

fn help_text() -> &'static str {
    r#"
Available commands:

    * help          Show help information

    * fep           Run a shell command inside every immediate subdirectory
                    of the working folder, in parallel.

                    Usage: `rat fep <<command>> [flags]`
                    Example: `rat fep git pull`

                    By default runs in the current working folder, or the
                    folder set with `cfg here`; override that for one call
                    with the `--local` flag.

                    fep's own flags (all optional):

                      --local             use CWD for this run, even if a
                                          default folder is configured
                      --recursive (--r)   walk the whole subtree instead of
                                          just the immediate subfolders, for
                                          monorepo-of-monorepos layouts;
                                          combine with --skip or `cfg ignore`
                                          to prune folders (e.g.
                                          node_modules) instead of walking
                                          into them
                      --concurrency-4     run at most 4 directories at once
                                          (default: number of CPUs)
                      --sync              run exactly one directory at a
                                          time (like --concurrency-1, but
                                          says what you mean); wins over
                                          --concurrency if both are given
                      --only-uk-fi        only run in subfolders that have a
                                          "-"-separated name component
                                          containing "uk" or "fi" (also
                                          accepts --only-uk,fi); matches by
                                          substring anywhere in the
                                          component, so "uk" also matches
                                          "ukpr" or "nlukx", not just a
                                          component that's exactly "uk"
                      --skip-priv-corp    skip subfolders that have a name
                                          component containing "priv" or
                                          "corp"; combines with --only
                                          instead of being ignored by it - a
                                          folder must satisfy --only (if
                                          given) AND not match --skip (if
                                          given)
                      --sustain           wait as long as it takes, ignoring
                                          any timeout
                      --timeout-30        timeout this run after 30 seconds,
                                          overriding the configured timeout
                                          (--timeout-0 means a 0-second
                                          timeout, NOT "disabled" - use
                                          --sustain or `cfg nto` for that)

                    IMPORTANT: rat parses these flags out of <<command>>
                    itself, before your command ever runs. Any `--word...`
                    you pass that starts with
                    local/skip/only/sustain/timeout/sync/recursive/r is
                    captured by rat instead of reaching your command, and
                    any other unrecognized `--flag` is silently dropped
                    rather than forwarded. So:

                      `rat fep git merge --skip-commit`
                      -> rat reads "--skip-commit" as its own --skip flag;
                         git never sees --skip-commit at all.

                    Single-dash flags (`-m`, `-rf`, `-n`, ...) are never
                    touched by rat and always reach your command untouched,
                    e.g. `rat fep git commit -m "message"` is safe.

    * pipe          Run an ordered pipeline of different commands, in
                    different directories, where a step can be backgrounded
                    and gated on readiness before the next one starts -
                    e.g. start a dev server, wait until it's actually up,
                    then start one that depends on it.

                    Usage: `rat pipe [step-flags] -- <command...>
                    [--then [step-flags] -- <command...> ...]`

                    Example (two dev servers, second waits on the first):
                    `rat pipe --dir studio --bg --ready-port 3333 \
                      -- npm run dev \
                      --then --dir blog -- npm run dev`

                    Everything before a step's own `--` is that step's
                    flags; everything after it, up to the next `--then` or
                    the end of the command line, is the literal command to
                    run - unrecognized flags before `--` are a hard error,
                    unlike fep's silent-drop.

                    Per-step flags (all optional):

                      --dir <path>          directory to run this step's
                                             command in (default: current
                                             directory)
                      --bg                   run this step in the
                                             background instead of waiting
                                             for it to exit; requires a
                                             readiness rule
                      --ready-port <n>       wait until something accepts a
                                             TCP connection on
                                             127.0.0.1:<n> before starting
                                             the next step
                      --ready-match <text>   wait until a line of this
                                             step's stdout/stderr contains
                                             <text>
                      --ready-timeout <secs> how long to wait for readiness
                                             before giving up (default: 30)

                    A non-backgrounded step blocks the pipeline until it
                    exits; a failing step (or a background step that never
                    becomes ready) tears down every step already started
                    and stops the pipeline. Once every declared step has
                    started, rat pipe supervises any still-running
                    background steps until Ctrl-C or one of them exits
                    unexpectedly, tearing all of them down either way.

    * cfg (config)
    cfg path        prints out config file's path
    cfg file        prints out config file's content

    cfg here        sets current working directory as a default working directory for future
                    uses with fep, untill it gets unset or new directory is set
    cfg away        unsets default working directory and allows running fep in current working directory

    cfg ignore      adds folders to the permanently ignored list
                    `rat cfg ignore .git .idea .vscode`
    cfg heed        removes folders from the permanently ignored list
                    `rat cfg heed .git .idea .vscode`
                    or
                    `rat cfg heed --all`

    cfg to          sets timeout in seconds
                    `rat cfg to 30`
                    or disables timeout if passed 0 - same as nto
                    `rat cfg to 0`
    cfg nto         disables timeout

"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_text_references_the_new_binary_name() {
        assert!(help_text().contains("rat fep"));
        assert!(help_text().contains("rat cfg ignore"));
        assert!(help_text().contains("rat cfg heed"));
        assert!(help_text().contains("rat cfg to"));
        assert!(help_text().contains("--recursive"));
        assert!(help_text().contains("rat pipe"));
        assert!(help_text().contains("--ready-port"));
        assert!(help_text().contains("--ready-match"));
        // catches leftover `ath <command>` invocations from the C# original
        // without false-positiving on "path", which legitimately contains "ath"
        assert!(!help_text().contains("`ath "));
    }
}
