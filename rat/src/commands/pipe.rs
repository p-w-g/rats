use crate::exec::pipeline::{self, Readiness, StepSpec};
use crate::exec::quoting;
use std::path::PathBuf;
use std::time::Duration;

/// See fep.rs's own `HELP_ALIASES` for why only a *whole* payload of one of
/// these counts as "show help" rather than checking anywhere in `args` -
/// this subcommand has its own raw grammar (see below) rather than sharing
/// `cli::parse_instance`, but the same ambiguity concern applies: a real
/// step could conceivably want to pass `-h` through to its own command.
const HELP_ALIASES: &[&str] = &["-h", "-help"];

const STEP_SEPARATOR: &str = "--then";
const DEFAULT_READY_TIMEOUT_SECS: u64 = 30;

/// Runs an ordered, readiness-gated pipeline of steps described entirely on
/// the command line:
///
///   rat pipe [step-flags] -- <command...> --then [step-flags] -- <command...> --then ...
///
/// Deliberately bypasses `cli::parse_instance` (see rat/src/cli/mod.rs):
/// that parser's dash-packed `--flag-v1-v2` grammar is fep/cfg-specific and
/// would corrupt a directory like `--dir-apps/sanity-studio` by splitting
/// on the path's own dashes. `pipe` needs its own small grammar instead.
pub fn run(args: &[String]) -> bool {
    if args.is_empty() || (args.len() == 1 && HELP_ALIASES.contains(&args[0].as_str())) {
        println!("{}", usage());
        return true;
    }

    match parse_steps(args) {
        Ok(steps) => pipeline::run(&steps),
        Err(e) => {
            println!("{e}\n\n{}", usage());
            false
        }
    }
}

fn usage() -> &'static str {
    "Usage: rat pipe [--dir <path>] [--bg --ready-port <n> | --ready-match <text>] \
[--ready-timeout <secs>] -- <command...> [--then [step-flags] -- <command...> ...]"
}

fn parse_steps(args: &[String]) -> Result<Vec<StepSpec>, String> {
    split_on_then(args)
        .iter()
        .enumerate()
        .map(|(i, chunk)| parse_step(i + 1, chunk))
        .collect()
}

/// Splits on the literal `--then` token. A command word that happens to be
/// exactly `--then` would be misread as a step boundary - an inherent
/// ambiguity of a token-based separator, same class of best-effort
/// trade-off `exec::quoting` already documents for shell-operator tokens.
fn split_on_then(args: &[String]) -> Vec<&[String]> {
    let mut chunks = Vec::new();
    let mut start = 0;
    for (i, arg) in args.iter().enumerate() {
        if arg == STEP_SEPARATOR {
            chunks.push(&args[start..i]);
            start = i + 1;
        }
    }
    chunks.push(&args[start..]);
    chunks
}

fn parse_step(step_number: usize, chunk: &[String]) -> Result<StepSpec, String> {
    let separator_index = chunk
        .iter()
        .position(|a| a == "--")
        .ok_or_else(|| format!("Step {step_number} is missing '--' before its command."))?;

    let flags = &chunk[..separator_index];
    let command_words = &chunk[separator_index + 1..];
    if command_words.is_empty() {
        return Err(format!("Step {step_number} has no command after '--'."));
    }

    let mut dir: Option<String> = None;
    let mut background = false;
    let mut ready_port: Option<String> = None;
    let mut ready_match: Option<String> = None;
    let mut ready_timeout: Option<String> = None;

    let mut i = 0;
    while i < flags.len() {
        match flags[i].as_str() {
            "--dir" => dir = Some(take_value(flags, &mut i, step_number, "--dir")?),
            "--bg" => {
                background = true;
                i += 1;
            }
            "--ready-port" => {
                ready_port = Some(take_value(flags, &mut i, step_number, "--ready-port")?)
            }
            "--ready-match" => {
                ready_match = Some(take_value(flags, &mut i, step_number, "--ready-match")?)
            }
            "--ready-timeout" => {
                ready_timeout = Some(take_value(flags, &mut i, step_number, "--ready-timeout")?)
            }
            other => return Err(format!("Step {step_number}: unrecognized flag '{other}'.")),
        }
    }

    let readiness = match (ready_port, ready_match) {
        (Some(_), Some(_)) => {
            return Err(format!(
                "Step {step_number}: --ready-port and --ready-match are mutually exclusive."
            ));
        }
        (Some(port), None) => {
            let port: u16 = port.parse().map_err(|_| {
                format!("Step {step_number}: --ready-port needs a valid port number, got '{port}'.")
            })?;
            Some(Readiness::Port(port))
        }
        (None, Some(text)) => Some(Readiness::Match(text)),
        (None, None) => None,
    };

    if background && readiness.is_none() {
        return Err(format!(
            "Step {step_number} has --bg but no readiness rule (--ready-port or --ready-match) \
             - rat wouldn't know when to move on to the next step."
        ));
    }
    if !background && readiness.is_some() {
        return Err(format!(
            "Step {step_number} has a readiness rule but isn't backgrounded (--bg) - \
             readiness only applies to background steps, rat already waits for a foreground \
             step to exit."
        ));
    }

    let ready_timeout = match ready_timeout {
        Some(secs) => {
            let secs: u64 = secs.parse().map_err(|_| {
                format!(
                    "Step {step_number}: --ready-timeout needs a whole number of seconds, got '{secs}'."
                )
            })?;
            Duration::from_secs(secs)
        }
        None => Duration::from_secs(DEFAULT_READY_TIMEOUT_SECS),
    };

    let dir = PathBuf::from(dir.unwrap_or_else(|| ".".to_string()));
    let label = dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("step {step_number}"));
    let command = quoting::build_command_line(command_words);

    Ok(StepSpec {
        label,
        dir,
        command,
        background,
        readiness,
        ready_timeout,
    })
}

fn take_value(
    flags: &[String],
    i: &mut usize,
    step_number: usize,
    flag_name: &str,
) -> Result<String, String> {
    let value = flags
        .get(*i + 1)
        .ok_or_else(|| format!("Step {step_number}: {flag_name} needs a value."))?
        .clone();
    *i += 2;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn split_on_then_splits_on_the_literal_separator() {
        let a = args(&[
            "--dir", "a", "--", "echo", "hi", "--then", "--", "echo", "bye",
        ]);
        let chunks = split_on_then(&a);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], &a[0..5]);
        assert_eq!(chunks[1], &a[6..9]);
    }

    #[test]
    fn split_on_then_with_no_separator_is_a_single_chunk() {
        let a = args(&["--", "echo", "hi"]);
        let chunks = split_on_then(&a);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], &a[..]);
    }

    #[test]
    fn parses_a_minimal_foreground_step() {
        let a = args(&["--", "npx", "testcafe"]);
        let steps = parse_steps(&a).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].command, "npx testcafe");
        assert_eq!(steps[0].dir, PathBuf::from("."));
        assert_eq!(steps[0].label, "step 1");
        assert!(!steps[0].background);
        assert_eq!(steps[0].readiness, None);
        assert_eq!(steps[0].ready_timeout, Duration::from_secs(30));
    }

    #[test]
    fn dir_flag_sets_directory_and_label() {
        let a = args(&["--dir", "apps/sanity-studio", "--", "npm", "run", "dev"]);
        let steps = parse_steps(&a).unwrap();
        assert_eq!(steps[0].dir, PathBuf::from("apps/sanity-studio"));
        // the directory's own dash must not get mis-split by pipe's parser,
        // unlike cli::parse_instance's dash-packed --flag-v1-v2 grammar.
        assert_eq!(steps[0].label, "sanity-studio");
    }

    #[test]
    fn bg_step_with_ready_port_parses() {
        let a = args(&[
            "--dir",
            "studio",
            "--bg",
            "--ready-port",
            "3333",
            "--",
            "npm",
            "run",
            "dev",
        ]);
        let steps = parse_steps(&a).unwrap();
        assert!(steps[0].background);
        assert_eq!(steps[0].readiness, Some(Readiness::Port(3333)));
    }

    #[test]
    fn bg_step_with_ready_match_parses() {
        let a = args(&["--bg", "--ready-match", "Local:", "--", "npm", "run", "dev"]);
        let steps = parse_steps(&a).unwrap();
        assert_eq!(
            steps[0].readiness,
            Some(Readiness::Match("Local:".to_string()))
        );
    }

    #[test]
    fn ready_timeout_flag_overrides_default() {
        let a = args(&[
            "--bg",
            "--ready-port",
            "80",
            "--ready-timeout",
            "5",
            "--",
            "npm",
            "run",
            "dev",
        ]);
        let steps = parse_steps(&a).unwrap();
        assert_eq!(steps[0].ready_timeout, Duration::from_secs(5));
    }

    #[test]
    fn multiple_steps_split_on_then() {
        let a = args(&[
            "--dir",
            "studio",
            "--bg",
            "--ready-port",
            "3333",
            "--",
            "npm",
            "run",
            "dev",
            "--then",
            "--dir",
            "blog",
            "--",
            "npm",
            "run",
            "dev",
        ]);
        let steps = parse_steps(&a).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].label, "studio");
        assert_eq!(steps[1].label, "blog");
        assert!(!steps[1].background);
    }

    #[test]
    fn missing_separator_is_an_error() {
        let a = args(&["--dir", "studio", "npm", "run", "dev"]);
        let err = parse_steps(&a).unwrap_err();
        assert!(err.contains("missing '--'"), "{err}");
    }

    #[test]
    fn empty_command_after_separator_is_an_error() {
        let a = args(&["--dir", "studio", "--"]);
        let err = parse_steps(&a).unwrap_err();
        assert!(err.contains("no command"), "{err}");
    }

    #[test]
    fn unrecognized_flag_is_a_hard_error_not_silently_dropped() {
        // Deliberate deviation from cli::parse_instance's silent-drop of
        // unknown flags: pipe's grammar is new/stricter, so a typo like
        // `--drr` should fail loudly, not vanish.
        let a = args(&["--drr", "studio", "--", "echo", "hi"]);
        let err = parse_steps(&a).unwrap_err();
        assert!(err.contains("unrecognized flag '--drr'"), "{err}");
    }

    #[test]
    fn bg_without_readiness_is_an_error() {
        let a = args(&["--bg", "--", "npm", "run", "dev"]);
        let err = parse_steps(&a).unwrap_err();
        assert!(err.contains("no readiness rule"), "{err}");
    }

    #[test]
    fn readiness_without_bg_is_an_error() {
        let a = args(&["--ready-port", "80", "--", "npm", "run", "dev"]);
        let err = parse_steps(&a).unwrap_err();
        assert!(err.contains("isn't backgrounded"), "{err}");
    }

    #[test]
    fn both_readiness_flags_together_is_an_error() {
        let a = args(&[
            "--bg",
            "--ready-port",
            "80",
            "--ready-match",
            "up",
            "--",
            "npm",
            "run",
            "dev",
        ]);
        let err = parse_steps(&a).unwrap_err();
        assert!(err.contains("mutually exclusive"), "{err}");
    }

    #[test]
    fn invalid_port_value_is_an_error() {
        let a = args(&[
            "--bg",
            "--ready-port",
            "not-a-port",
            "--",
            "npm",
            "run",
            "dev",
        ]);
        let err = parse_steps(&a).unwrap_err();
        assert!(err.contains("valid port number"), "{err}");
    }

    #[test]
    fn invalid_timeout_value_is_an_error() {
        let a = args(&[
            "--bg",
            "--ready-port",
            "80",
            "--ready-timeout",
            "soon",
            "--",
            "npm",
            "run",
            "dev",
        ]);
        let err = parse_steps(&a).unwrap_err();
        assert!(err.contains("whole number of seconds"), "{err}");
    }

    #[test]
    fn flag_missing_its_value_is_an_error() {
        // The separator is found first (scanning the whole chunk), so
        // `--dir` ends up with nothing before it in the flags slice.
        let a = args(&["--dir", "--", "echo", "hi"]);
        let err = parse_steps(&a).unwrap_err();
        assert!(err.contains("--dir needs a value"), "{err}");
    }

    #[test]
    fn no_args_at_all_shows_usage_and_counts_as_success() {
        assert!(run(&[]));
    }

    #[test]
    fn help_alias_shows_usage_and_counts_as_success() {
        assert!(run(&args(&["-h"])));
    }
}
