use std::process::Command;

fn rat_help_output() -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_rat"))
        .arg("help")
        .output()
        .expect("failed to run `rat help`");
    String::from_utf8(output.stdout).expect("help output was not valid UTF-8")
}

fn readme() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))
        .expect("failed to read README.md")
}

/// `rat help` quotes with `"..."`, README with `` `...` `` (and sometimes
/// `**...**`) - strip all three plus collapse whitespace so the same fact
/// compares equal regardless of which doc's quoting style it's written in.
fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '`' | '"' | '*'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Facts that must be stated - in these exact words, modulo quoting style -
/// in both `rat help` and README.md. Add to this list whenever a fact
/// exists in one doc and is load-bearing enough that the other doc silently
/// missing it would mislead a reader, not for every wording variation (the
/// two docs intentionally paraphrase each other in most places; README in
/// particular keeps some flag rows terse with a "see below" pointer to a
/// fuller prose section, e.g. --recursive/--sync/--skip - that's a feature,
/// not drift).
///
/// This list exists because it already happened once: help text documented
/// that --only accepts a comma-separated alias (--only-uk,fi) alongside its
/// dash form, and README's flag table silently didn't.
const CROSS_DOC_FACTS: &[&str] = &[
    "name component containing uk or fi (also accepts --only-uk,fi)",
    "run at most 4 directories at once (default: number of cpus)",
    "wait as long as it takes, ignoring any timeout",
];

#[test]
fn help_and_readme_state_the_same_load_bearing_facts() {
    let help = normalize(&rat_help_output());
    let doc = normalize(&readme());

    for fact in CROSS_DOC_FACTS {
        let fact = normalize(fact);
        assert!(
            help.contains(&fact),
            "`rat help` is missing a fact that README.md states: {fact:?}"
        );
        assert!(
            doc.contains(&fact),
            "README.md is missing a fact that `rat help` states: {fact:?}"
        );
    }
}
