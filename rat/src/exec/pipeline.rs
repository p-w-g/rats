use crate::exec::process;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Child;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// One step of a `rat pipe` run: a command to run in a directory, either
/// blocking the pipeline until it exits (the default) or backgrounded and
/// gated by a readiness rule before the next step starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepSpec {
    /// Used to prefix this step's streamed output and to name it in
    /// error/status messages - the last path component of `dir`, or
    /// "step N" when `dir` is just ".".
    pub label: String,
    pub dir: PathBuf,
    pub command: String,
    pub background: bool,
    /// `Some` iff `background` is true - enforced by the parser in
    /// `commands::pipe`, not re-validated here.
    pub readiness: Option<Readiness>,
    pub ready_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    Port(u16),
    Match(String),
}

/// How often readiness/liveness is polled. Mirrors the poll interval
/// `exec::process::wait_with_timeout` already uses for the same kind of
/// "no blocking primitive for this, so poll" wait.
const POLL_INTERVAL: Duration = Duration::from_millis(75);

struct RunningStep {
    label: String,
    child: Child,
}

/// Runs `steps` in order, tearing every backgrounded step down on failure,
/// on an unexpected child death, or on Ctrl-C. Returns whether the run
/// completed as intended: a deliberate Ctrl-C counts as success (the user
/// asked for exactly what happened - everything stopped cleanly), matching
/// this binary's existing plain true/false-only exit convention (see
/// `commands::fep::run_parallel`) rather than introducing a new exit code.
pub fn run(steps: &[StepSpec]) -> bool {
    let interrupted = Arc::new(AtomicBool::new(false));
    {
        let interrupted = Arc::clone(&interrupted);
        // set_handler can only meaningfully be installed once per process;
        // `rat pipe` is the only caller today. A failure to install (e.g.
        // a signal handler already set by an embedding process) degrades
        // to "Ctrl-C behaves like it always did for a plain child process
        // tree" rather than crashing the run over it.
        let _ = ctrlc::set_handler(move || {
            interrupted.store(true, Ordering::SeqCst);
        });
    }
    run_with_interrupt_flag(steps, &interrupted)
}

/// Split out from `run` so tests can simulate an interrupt by flipping the
/// flag directly instead of delivering a real OS signal, which isn't
/// something `cargo test` can portably and deterministically drive.
fn run_with_interrupt_flag(steps: &[StepSpec], interrupted: &AtomicBool) -> bool {
    let mut running: Vec<RunningStep> = Vec::new();
    let mut last_foreground_success = true;

    for step in steps {
        if interrupted.load(Ordering::SeqCst) {
            teardown(&mut running);
            return true;
        }

        println!(
            "Starting '{}' in {}{}",
            step.command,
            step.dir.display(),
            if step.background { " (background)" } else { "" }
        );

        let mut child =
            match process::spawn_shell(&step.command, &step.dir, process::SHELL_CANDIDATES) {
                Ok(child) => child,
                Err(e) => {
                    println!("Couldn't start step '{}': {e}", step.label);
                    teardown(&mut running);
                    return false;
                }
            };

        let matched = Arc::new(AtomicBool::new(false));
        let match_text: Option<Arc<str>> = match &step.readiness {
            Some(Readiness::Match(text)) => Some(Arc::from(text.as_str())),
            _ => None,
        };
        let stdout = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");
        spawn_line_reader(
            step.label.clone(),
            stdout,
            match_text.clone(),
            Arc::clone(&matched),
        );
        spawn_line_reader(step.label.clone(), stderr, match_text, Arc::clone(&matched));

        if step.background {
            let readiness = step
                .readiness
                .as_ref()
                .expect("a background step always carries a readiness rule");
            match wait_until_ready(
                readiness,
                step.ready_timeout,
                &matched,
                &mut child,
                &mut running,
                interrupted,
            ) {
                ReadyOutcome::Ready => {
                    println!("Step '{}' is ready", step.label);
                    running.push(RunningStep {
                        label: step.label.clone(),
                        child,
                    });
                }
                ReadyOutcome::TimedOut => {
                    println!(
                        "Step '{}' did not become ready within {} seconds",
                        step.label,
                        step.ready_timeout.as_secs()
                    );
                    process::kill_process_tree(&mut child);
                    let _ = child.wait();
                    teardown(&mut running);
                    return false;
                }
                ReadyOutcome::SelfDied => {
                    println!("Step '{}' exited before becoming ready", step.label);
                    teardown(&mut running);
                    return false;
                }
                ReadyOutcome::UpstreamDied(dead_label) => {
                    println!(
                        "Step '{dead_label}' exited unexpectedly while waiting for '{}' to become ready",
                        step.label
                    );
                    process::kill_process_tree(&mut child);
                    let _ = child.wait();
                    teardown(&mut running);
                    return false;
                }
                ReadyOutcome::Interrupted => {
                    process::kill_process_tree(&mut child);
                    let _ = child.wait();
                    teardown(&mut running);
                    return true;
                }
            }
        } else {
            match wait_for_foreground_exit(&mut child, &mut running, interrupted) {
                ForegroundOutcome::Exited(success) => {
                    last_foreground_success = success;
                    if !success {
                        println!("Step '{}' failed", step.label);
                        teardown(&mut running);
                        return false;
                    }
                }
                ForegroundOutcome::UpstreamDied(dead_label) => {
                    println!(
                        "Step '{dead_label}' exited unexpectedly while '{}' was running",
                        step.label
                    );
                    process::kill_process_tree(&mut child);
                    let _ = child.wait();
                    teardown(&mut running);
                    return false;
                }
                ForegroundOutcome::Interrupted => {
                    process::kill_process_tree(&mut child);
                    let _ = child.wait();
                    teardown(&mut running);
                    return true;
                }
            }
        }
    }

    // Whether to keep supervising depends on the *last declared step*, not
    // just whether `running` happens to be non-empty: a pipeline can be
    // background-then-foreground (build-and-serve, then a foreground test
    // run against it), where an earlier background step is still alive but
    // the pipeline's natural end is the foreground step that already
    // exited. Using `running.is_empty()` here instead would wrongly fall
    // through to supervising forever in exactly that shape - caught by
    // hand-testing the build->serve->test case, where `rat pipe` hung
    // instead of tearing the server down once the foreground step finished.
    let last_step_is_background = steps.last().is_some_and(|s| s.background);
    if !last_step_is_background {
        teardown(&mut running);
        return last_foreground_success;
    }

    // The last declared step is itself backgrounded, so there's no natural
    // end - this is the steady state for an all-background pipeline (e.g.
    // two dev servers): supervise until Ctrl-C or an unexpected death.
    let outcome = supervise(&mut running, interrupted);
    teardown(&mut running);
    match outcome {
        SupervisionOutcome::Interrupted => true,
        SupervisionOutcome::Died(label) => {
            println!("Step '{label}' exited unexpectedly");
            false
        }
    }
}

enum ReadyOutcome {
    Ready,
    TimedOut,
    SelfDied,
    UpstreamDied(String),
    Interrupted,
}

/// Waits for a backgrounded step's own readiness rule to be satisfied,
/// while also watching for the kind of failure that would otherwise strand
/// the wait for its full timeout: the step's own process dying before it
/// ever becomes ready, an already-confirmed-ready earlier step dying while
/// this one waits (it may depend on it), or the user hitting Ctrl-C.
fn wait_until_ready(
    readiness: &Readiness,
    timeout: Duration,
    matched: &AtomicBool,
    current: &mut Child,
    running: &mut [RunningStep],
    interrupted: &AtomicBool,
) -> ReadyOutcome {
    let start = Instant::now();
    loop {
        if interrupted.load(Ordering::SeqCst) {
            return ReadyOutcome::Interrupted;
        }
        if matches!(current.try_wait(), Ok(Some(_))) {
            return ReadyOutcome::SelfDied;
        }
        if let Some(label) = find_dead(running) {
            return ReadyOutcome::UpstreamDied(label);
        }

        let ready = match readiness {
            Readiness::Port(port) => tcp_port_open(*port),
            Readiness::Match(_) => matched.load(Ordering::SeqCst),
        };
        if ready {
            return ReadyOutcome::Ready;
        }
        if start.elapsed() >= timeout {
            return ReadyOutcome::TimedOut;
        }
        thread::sleep(POLL_INTERVAL);
    }
}

enum ForegroundOutcome {
    Exited(bool),
    UpstreamDied(String),
    Interrupted,
}

/// Waits for a foreground step to exit, while also watching already-running
/// background steps for an unexpected death - a foreground step can depend
/// on one just as much as a later backgrounded one can.
fn wait_for_foreground_exit(
    current: &mut Child,
    running: &mut [RunningStep],
    interrupted: &AtomicBool,
) -> ForegroundOutcome {
    loop {
        if interrupted.load(Ordering::SeqCst) {
            return ForegroundOutcome::Interrupted;
        }
        if let Ok(Some(status)) = current.try_wait() {
            return ForegroundOutcome::Exited(status.success());
        }
        if let Some(label) = find_dead(running) {
            return ForegroundOutcome::UpstreamDied(label);
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Returns the label of the first already-started background step that has
/// exited on its own (as opposed to being killed by `teardown`), if any.
fn find_dead(running: &mut [RunningStep]) -> Option<String> {
    for r in running.iter_mut() {
        if matches!(r.child.try_wait(), Ok(Some(_))) {
            return Some(r.label.clone());
        }
    }
    None
}

fn supervise(running: &mut [RunningStep], interrupted: &AtomicBool) -> SupervisionOutcome {
    loop {
        if interrupted.load(Ordering::SeqCst) {
            return SupervisionOutcome::Interrupted;
        }
        if let Some(label) = find_dead(running) {
            return SupervisionOutcome::Died(label);
        }
        thread::sleep(POLL_INTERVAL);
    }
}

enum SupervisionOutcome {
    Interrupted,
    Died(String),
}

/// Kills every still-running background step's whole process tree. Reuses
/// `process::kill_process_tree` (already handles late-forking grandchildren
/// via its multi-pass descendant scan) rather than a plain `child.kill()`,
/// which - as documented on that function - only reaches the direct child.
/// Calling this on a step that already exited on its own is harmless: the
/// underlying `kill -KILL`/`taskkill` just finds nothing to kill.
fn teardown(running: &mut [RunningStep]) {
    for r in running.iter_mut() {
        process::kill_process_tree(&mut r.child);
        let _ = r.child.wait();
    }
}

fn tcp_port_open(port: u16) -> bool {
    TcpStream::connect(("127.0.0.1", port)).is_ok()
}

/// Streams `stream` line by line, printing each as `[label] <line>` as it
/// arrives rather than buffering to completion like
/// `exec::process::run_command` does - that model doesn't fit a step that
/// may never exit. When `match_text` is set, the first line containing it
/// flips `matched`, which `wait_until_ready` polls for `Readiness::Match`.
fn spawn_line_reader<R: Read + Send + 'static>(
    label: String,
    stream: R,
    match_text: Option<Arc<str>>,
    matched: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines().map_while(Result::ok) {
            println!("[{label}] {line}");
            if let Some(text) = &match_text {
                if !matched.load(Ordering::SeqCst) && line.contains(text.as_ref()) {
                    matched.store(true, Ordering::SeqCst);
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[cfg(target_os = "windows")]
    fn sleep_command(seconds: u32) -> String {
        format!("ping -n {} 127.0.0.1 > nul", seconds + 1)
    }
    #[cfg(not(target_os = "windows"))]
    fn sleep_command(seconds: u32) -> String {
        format!("sleep {seconds}")
    }

    #[cfg(target_os = "windows")]
    fn noop_command() -> String {
        "exit 0".to_string()
    }
    #[cfg(not(target_os = "windows"))]
    fn noop_command() -> String {
        "true".to_string()
    }

    #[cfg(target_os = "windows")]
    fn failing_command() -> String {
        "exit 1".to_string()
    }
    #[cfg(not(target_os = "windows"))]
    fn failing_command() -> String {
        "false".to_string()
    }

    #[cfg(target_os = "windows")]
    fn echo_ready_then_sleep_command(seconds: u32) -> String {
        format!("echo READY & ping -n {} 127.0.0.1 > nul", seconds + 1)
    }
    #[cfg(not(target_os = "windows"))]
    fn echo_ready_then_sleep_command(seconds: u32) -> String {
        format!("echo READY && sleep {seconds}")
    }

    fn spawn(command: &str) -> Child {
        let dir = std::env::temp_dir();
        process::spawn_shell(command, &dir, process::SHELL_CANDIDATES).unwrap()
    }

    fn free_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    }

    #[test]
    fn tcp_port_open_is_false_when_nothing_is_listening() {
        let port = free_port(); // bound then immediately dropped, freeing it
        assert!(!tcp_port_open(port));
    }

    #[test]
    fn tcp_port_open_is_true_once_something_is_listening() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(tcp_port_open(port));
        drop(listener);
    }

    #[test]
    fn wait_until_ready_succeeds_once_the_port_opens() {
        let port = free_port();
        // Bind the listener only after the wait has had time to poll at
        // least once with nothing there yet, proving this actually retries
        // rather than only checking a single time.
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            let _listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
            thread::sleep(Duration::from_secs(2));
        });

        let mut current = spawn(&sleep_command(5));
        let matched = AtomicBool::new(false);
        let interrupted = AtomicBool::new(false);
        let outcome = wait_until_ready(
            &Readiness::Port(port),
            Duration::from_secs(5),
            &matched,
            &mut current,
            &mut [],
            &interrupted,
        );
        assert!(matches!(outcome, ReadyOutcome::Ready));
        let _ = current.kill();
        let _ = current.wait();
    }

    #[test]
    fn wait_until_ready_times_out_when_the_port_never_opens() {
        let port = free_port();
        let mut current = spawn(&sleep_command(5));
        let matched = AtomicBool::new(false);
        let interrupted = AtomicBool::new(false);
        let outcome = wait_until_ready(
            &Readiness::Port(port),
            Duration::from_millis(200),
            &matched,
            &mut current,
            &mut [],
            &interrupted,
        );
        assert!(matches!(outcome, ReadyOutcome::TimedOut));
        let _ = current.kill();
        let _ = current.wait();
    }

    #[test]
    fn wait_until_ready_succeeds_once_matched_flag_is_set() {
        let mut current = spawn(&sleep_command(5));
        let matched = AtomicBool::new(true);
        let interrupted = AtomicBool::new(false);
        let outcome = wait_until_ready(
            &Readiness::Match("ready".to_string()),
            Duration::from_secs(5),
            &matched,
            &mut current,
            &mut [],
            &interrupted,
        );
        assert!(matches!(outcome, ReadyOutcome::Ready));
        let _ = current.kill();
        let _ = current.wait();
    }

    #[test]
    fn wait_until_ready_reports_self_death_instead_of_waiting_out_the_timeout() {
        let mut current = spawn(&noop_command()); // exits almost immediately
        let matched = AtomicBool::new(false);
        let interrupted = AtomicBool::new(false);
        let start = Instant::now();
        let outcome = wait_until_ready(
            &Readiness::Match("never".to_string()),
            Duration::from_secs(30),
            &matched,
            &mut current,
            &mut [],
            &interrupted,
        );
        assert!(matches!(outcome, ReadyOutcome::SelfDied));
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "should fail fast on self-death, not wait out the 30s timeout"
        );
    }

    #[test]
    fn wait_until_ready_reports_upstream_death_instead_of_waiting_out_the_timeout() {
        let mut current = spawn(&sleep_command(5));
        let mut running = vec![RunningStep {
            label: "upstream".to_string(),
            child: spawn(&noop_command()), // exits almost immediately
        }];
        let matched = AtomicBool::new(false);
        let interrupted = AtomicBool::new(false);
        let start = Instant::now();
        let outcome = wait_until_ready(
            &Readiness::Match("never".to_string()),
            Duration::from_secs(30),
            &matched,
            &mut current,
            &mut running,
            &interrupted,
        );
        match outcome {
            ReadyOutcome::UpstreamDied(label) => assert_eq!(label, "upstream"),
            _ => panic!("expected UpstreamDied"),
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "should fail fast on upstream death, not wait out the 30s timeout"
        );
        let _ = current.kill();
        let _ = current.wait();
    }

    #[test]
    fn wait_until_ready_honors_interrupt() {
        let mut current = spawn(&sleep_command(5));
        let matched = AtomicBool::new(false);
        let interrupted = AtomicBool::new(true);
        let outcome = wait_until_ready(
            &Readiness::Match("never".to_string()),
            Duration::from_secs(30),
            &matched,
            &mut current,
            &mut [],
            &interrupted,
        );
        assert!(matches!(outcome, ReadyOutcome::Interrupted));
        let _ = current.kill();
        let _ = current.wait();
    }

    #[test]
    fn wait_for_foreground_exit_reports_success_and_failure() {
        let interrupted = AtomicBool::new(false);

        let mut ok = spawn(&noop_command());
        assert!(matches!(
            wait_for_foreground_exit(&mut ok, &mut [], &interrupted),
            ForegroundOutcome::Exited(true)
        ));

        let mut bad = spawn(&failing_command());
        assert!(matches!(
            wait_for_foreground_exit(&mut bad, &mut [], &interrupted),
            ForegroundOutcome::Exited(false)
        ));
    }

    #[test]
    fn wait_for_foreground_exit_reports_upstream_death() {
        let mut current = spawn(&sleep_command(5));
        let mut running = vec![RunningStep {
            label: "upstream".to_string(),
            child: spawn(&noop_command()),
        }];
        let interrupted = AtomicBool::new(false);
        match wait_for_foreground_exit(&mut current, &mut running, &interrupted) {
            ForegroundOutcome::UpstreamDied(label) => assert_eq!(label, "upstream"),
            ForegroundOutcome::Exited(_) => panic!("expected UpstreamDied, got Exited"),
            ForegroundOutcome::Interrupted => panic!("expected UpstreamDied, got Interrupted"),
        }
        let _ = current.kill();
        let _ = current.wait();
    }

    #[test]
    fn find_dead_finds_an_exited_step_but_not_a_live_one() {
        let mut running = vec![
            RunningStep {
                label: "alive".to_string(),
                child: spawn(&sleep_command(5)),
            },
            RunningStep {
                label: "dead".to_string(),
                child: spawn(&noop_command()),
            },
        ];
        // give the "dead" one a moment to actually exit
        thread::sleep(Duration::from_millis(200));
        assert_eq!(find_dead(&mut running), Some("dead".to_string()));

        for r in running.iter_mut() {
            let _ = r.child.kill();
            let _ = r.child.wait();
        }
    }

    #[test]
    fn teardown_kills_every_still_running_step() {
        let mut running = vec![
            RunningStep {
                label: "a".to_string(),
                child: spawn(&sleep_command(30)),
            },
            RunningStep {
                label: "b".to_string(),
                child: spawn(&sleep_command(30)),
            },
        ];
        teardown(&mut running);
        for r in running.iter_mut() {
            // Already reaped by teardown's own wait(); confirm nothing is
            // left running rather than asserting on the (platform-specific)
            // exact status a killed process reports.
            assert!(matches!(r.child.try_wait(), Ok(Some(_)) | Err(_)));
        }
    }

    #[test]
    fn run_with_interrupt_flag_tears_down_and_reports_success_when_already_interrupted() {
        // Simulates Ctrl-C having already arrived before the pipeline even
        // starts its first step - the flag is the seam ctrlc's handler
        // writes to; flipping it directly avoids depending on real OS
        // signal delivery inside a test.
        let steps = vec![StepSpec {
            label: "never-runs".to_string(),
            dir: std::env::temp_dir(),
            command: noop_command(),
            background: false,
            readiness: None,
            ready_timeout: Duration::from_secs(5),
        }];
        let interrupted = AtomicBool::new(true);
        assert!(run_with_interrupt_flag(&steps, &interrupted));
    }

    #[test]
    fn run_with_interrupt_flag_runs_foreground_steps_in_order_and_reports_failure() {
        let interrupted = AtomicBool::new(false);
        let steps = vec![
            StepSpec {
                label: "ok".to_string(),
                dir: std::env::temp_dir(),
                command: noop_command(),
                background: false,
                readiness: None,
                ready_timeout: Duration::from_secs(5),
            },
            StepSpec {
                label: "boom".to_string(),
                dir: std::env::temp_dir(),
                command: failing_command(),
                background: false,
                readiness: None,
                ready_timeout: Duration::from_secs(5),
            },
        ];
        assert!(!run_with_interrupt_flag(&steps, &interrupted));
    }

    #[test]
    fn run_with_interrupt_flag_gates_a_background_step_on_its_own_readiness() {
        let interrupted = AtomicBool::new(false);
        let port = free_port();
        let steps = vec![
            StepSpec {
                label: "server".to_string(),
                dir: std::env::temp_dir(),
                command: sleep_command(5),
                background: true,
                readiness: Some(Readiness::Port(port)),
                ready_timeout: Duration::from_millis(300),
            },
            StepSpec {
                label: "never-ready".to_string(),
                dir: std::env::temp_dir(),
                command: noop_command(),
                background: false,
                readiness: None,
                ready_timeout: Duration::from_secs(5),
            },
        ];
        // Nothing ever listens on `port`, so the background step should
        // time out and the pipeline should fail without running the second
        // step at all.
        assert!(!run_with_interrupt_flag(&steps, &interrupted));
    }

    #[test]
    fn run_with_interrupt_flag_tears_down_a_background_step_once_the_final_foreground_step_exits() {
        // Regression test for the build-and-serve-then-test shape (a
        // background step followed by a foreground one): using
        // `running.is_empty()` to decide whether to keep supervising was
        // wrong here, since the background "server" step is still alive
        // when the loop ends even though the pipeline's natural end is the
        // foreground step that already exited - caught by hand-testing the
        // real case, where this hung supervising forever instead of
        // tearing "server" down and returning.
        let interrupted = AtomicBool::new(false);
        let steps = vec![
            StepSpec {
                label: "server".to_string(),
                dir: std::env::temp_dir(),
                command: echo_ready_then_sleep_command(30),
                background: true,
                readiness: Some(Readiness::Match("READY".to_string())),
                ready_timeout: Duration::from_secs(5),
            },
            StepSpec {
                label: "test".to_string(),
                dir: std::env::temp_dir(),
                command: noop_command(),
                background: false,
                readiness: None,
                ready_timeout: Duration::from_secs(5),
            },
        ];

        let start = Instant::now();
        let success = run_with_interrupt_flag(&steps, &interrupted);
        assert!(success);
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "pipeline should end once the final foreground step exits, not supervise \
             forever just because an earlier background step is still alive"
        );
    }
}
