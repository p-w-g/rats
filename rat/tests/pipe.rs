use std::net::TcpListener;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

fn rat() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rat"))
}

/// Splits a hand-written shell command (built the same way
/// exec::process's own tests build one) into separate argv words, the way
/// a real shell would before `rat` ever sees it. None of the commands used
/// here contain a token with meaningful embedded whitespace, so rejoining
/// via exec::quoting::build_command_line reproduces the original string
/// byte-for-byte - passing the whole string as a single argv element
/// instead would make quoting single-quote it wholesale, turning
/// shell-syntax like `(...) & wait` into inert literal text.
fn argv(command: &str) -> Vec<&str> {
    command.split_whitespace().collect()
}

/// A port nothing is listening on. Bound then immediately dropped, same
/// trick `exec::pipeline`'s own unit tests use, so the OS won't hand the
/// number back out to something else for the short window this test needs.
fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

#[cfg(target_os = "windows")]
fn failing_command() -> &'static str {
    "exit 1"
}
#[cfg(not(target_os = "windows"))]
fn failing_command() -> &'static str {
    "false"
}

/// Prints "UP" immediately (readiness signal), then - mirroring
/// exec::process's own `background_grandchild_then_touch` regression test -
/// backgrounds a grandchild that sleeps briefly and then touches `marker`.
/// If teardown only killed the direct shell (not the whole process tree),
/// the grandchild survives and writes the marker anyway.
#[cfg(target_os = "windows")]
fn signal_up_then_background_touch(marker: &Path) -> String {
    format!(
        "echo UP & start /B /WAIT cmd /c \"ping -n 5 127.0.0.1 > nul && echo done > {}\"",
        marker.display()
    )
}
#[cfg(not(target_os = "windows"))]
fn signal_up_then_background_touch(marker: &Path) -> String {
    format!("echo UP && (sleep 0.5 && touch {}) & wait", marker.display())
}

#[cfg(target_os = "windows")]
fn grandchild_settle_time() -> Duration {
    Duration::from_secs(5)
}
#[cfg(not(target_os = "windows"))]
fn grandchild_settle_time() -> Duration {
    Duration::from_millis(700)
}

#[test]
fn no_args_prints_usage_and_exits_zero() {
    let output = rat().arg("pipe").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: rat pipe"), "{stdout}");
    assert!(output.status.success());
}

#[test]
fn help_alias_prints_usage_and_exits_zero() {
    let output = rat().args(["pipe", "-h"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: rat pipe"), "{stdout}");
    assert!(output.status.success());
}

#[test]
fn missing_separator_reports_error_and_exits_nonzero() {
    let output = rat().args(["pipe", "--dir", "x", "echo", "hi"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("missing '--'"), "{stdout}");
    assert!(!output.status.success());
}

#[test]
fn bg_without_readiness_reports_error_and_exits_nonzero() {
    let output = rat()
        .args(["pipe", "--bg", "--", "echo", "hi"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("no readiness rule"), "{stdout}");
    assert!(!output.status.success());
}

#[test]
fn runs_a_single_foreground_step() {
    let output = rat().args(["pipe", "--", "echo", "hello"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[step 1] hello"), "{stdout}");
    assert!(output.status.success());
}

#[test]
fn second_step_does_not_start_until_the_first_becomes_ready() {
    // Step 1 backgrounds, signals ready via a stdout match, then exits
    // shortly after on its own - so the whole `rat pipe` invocation ends
    // naturally (via the "background step died while supervising" path)
    // instead of hanging forever waiting for a Ctrl-C a test can't send.
    let mut cmd = rat();
    cmd.args(["pipe", "--bg", "--ready-match", "UP", "--"]);
    cmd.args(argv("echo UP && sleep 0.3"));
    cmd.args(["--then", "--", "echo", "second"]);
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let ready_at = stdout.find("] UP").expect("readiness line missing");
    let second_started_at = stdout
        .find("Starting 'echo second'")
        .expect("second step never started");
    assert!(
        second_started_at > ready_at,
        "second step must start after the first signals ready, got:\n{stdout}"
    );
    assert!(stdout.contains("[step 2] second"), "{stdout}");
}

#[test]
fn readiness_timeout_fails_the_pipeline_without_running_the_next_step() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("should-not-exist");
    let port = free_port(); // nothing ever listens on it

    let mut cmd = rat();
    cmd.args([
        "pipe",
        "--bg",
        "--ready-port",
        &port.to_string(),
        "--ready-timeout",
        "1",
        "--",
    ]);
    cmd.args(argv("sleep 5"));
    cmd.args(["--then", "--"]);
    cmd.args(argv(&format!("touch {}", marker.display())));
    let output = cmd.output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success());
    assert!(stdout.contains("did not become ready"), "{stdout}");
    assert!(
        !marker.exists(),
        "the second step must never run when the first step's readiness times out"
    );
}

#[test]
fn a_successful_foreground_step_tears_down_an_earlier_background_step_instead_of_supervising_forever()
{
    // Regression test for the build-and-serve-then-test shape: the
    // pipeline's last declared step is foreground, so once it exits
    // successfully the whole run should end and tear the still-alive
    // background step down - not fall through to supervising forever just
    // because that step happens to still be running. Caught by hand-testing
    // the real case, where `rat pipe` hung instead of returning.
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("marker");

    let mut cmd = rat();
    cmd.args(["pipe", "--bg", "--ready-match", "UP", "--"]);
    cmd.args(argv(&signal_up_then_background_touch(&marker)));
    cmd.args(["--then", "--", "echo", "done"]);

    let start = std::time::Instant::now();
    let output = cmd.output().unwrap();
    let elapsed = start.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "{stdout}");
    assert!(
        elapsed < Duration::from_secs(3),
        "should end as soon as the foreground step exits, not wait around; took {elapsed:?}"
    );

    std::thread::sleep(grandchild_settle_time());
    assert!(
        !marker.exists(),
        "the background step's grandchild survived even though the pipeline ended successfully"
    );
}

#[test]
fn a_failing_foreground_step_tears_down_the_whole_process_tree_of_earlier_background_steps() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("marker");

    let mut cmd = rat();
    cmd.args(["pipe", "--bg", "--ready-match", "UP", "--"]);
    cmd.args(argv(&signal_up_then_background_touch(&marker)));
    cmd.args(["--then", "--"]);
    cmd.args(argv(failing_command()));
    let output = cmd.output().unwrap();

    assert!(!output.status.success());

    // Give the grandchild more time than its own sleep needs; if teardown
    // only reached the direct shell (not the whole tree), it survives and
    // writes the marker anyway, well after rat has already returned - same
    // reasoning as exec::process's own kill_process_tree regression test.
    std::thread::sleep(grandchild_settle_time());
    assert!(
        !marker.exists(),
        "a backgrounded step's grandchild process survived pipeline teardown"
    );
}
