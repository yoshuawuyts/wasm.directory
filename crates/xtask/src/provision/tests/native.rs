//! Exercise native subprocess IO using this test binary, never az or azd.

use std::io::{self, Read as _, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::{PASSWORD, saved, values};
use crate::provision::host::{Disclosure, Host as _, Input};
use crate::provision::process::{self, Stream};
use crate::provision::runtime::Runtime;

const MODE: &str = "XTASK_PROVISION_FIXTURE_MODE";
const FIXTURE: &str = "provision::tests::native::subprocess_fixture";

fn arguments() -> Vec<String> {
    let binary = std::env::current_exe().expect("locate this test binary");
    vec![
        binary
            .into_os_string()
            .into_string()
            .expect("UTF-8 test binary path"),
        "--exact".into(),
        FIXTURE.into(),
        "--ignored".into(),
        "--nocapture".into(),
        "--test-threads=1".into(),
    ]
}

pub(in crate::provision) fn command(mode: &str) -> Command {
    let arguments = arguments();
    let borrowed: Vec<_> = arguments.iter().map(String::as_str).collect();
    let mut command = process::command(&borrowed).expect("construct fixture process");
    command.env(MODE, mode);
    command
}

#[test]
fn provision_command_uses_confirmed_values_without_secret_arguments() {
    let directory = tempfile::tempdir().expect("create fixture root");
    let runtime = Runtime::with_inputs(
        directory.path().to_owned(),
        values(&[("POSTGRES_ADMIN_PASSWORD", "")]),
    );
    let settings = saved("test-env");
    let command = runtime
        .provision_command("test-env", &settings)
        .expect("construct provision command without running it");
    assert_eq!(command.get_program(), "azd");
    assert_eq!(command.get_current_dir(), Some(directory.path()));
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(
        args,
        ["provision", "--environment", "test-env", "--no-prompt"]
    );
    for (key, value) in &settings {
        let actual = command
            .get_envs()
            .find_map(|(name, value)| (name == std::ffi::OsStr::new(key)).then_some(value));
        assert_eq!(actual, Some(Some(std::ffi::OsStr::new(value))));
    }
    assert!(!format!("{args:?}").contains(PASSWORD));
}

#[test]
fn confirmed_defaults_remove_empty_inherited_overrides() {
    let directory = tempfile::tempdir().expect("create fixture root");
    let inputs = values(&[
        ("AZURE_RESOURCE_GROUP", ""),
        ("POSTGRES_ADMIN_LOGIN", ""),
        ("POSTGRES_DB", ""),
        ("CUSTOM_DOMAIN_NAME", ""),
    ]);
    let runtime = Runtime::with_inputs(directory.path().to_owned(), inputs.clone());
    let mut settings = saved("test-env");
    settings.retain(|key, _| !inputs.contains_key(key));
    let command = runtime
        .provision_command("test-env", &settings)
        .expect("construct the command using confirmed defaults");
    for key in inputs.keys() {
        let (_, value) = command
            .get_envs()
            .find(|(name, _)| *name == std::ffi::OsStr::new(key))
            .expect("the inherited override is explicitly removed");
        assert!(value.is_none());
    }
}

#[test]
fn drains_both_pipes_without_deadlock() {
    let mut stdout = String::new();
    let mut stderr = String::new();
    let status = process::run(command("burst"), &AtomicBool::new(false), |stream, line| {
        match stream {
            Stream::Stdout => stdout.push_str(line),
            Stream::Stderr => stderr.push_str(line),
        }
        Ok(())
    })
    .expect("fixture pipes are drained");
    assert!(status.success());
    assert_eq!(stdout.matches("fixture-out").count(), 4096);
    assert_eq!(stderr.matches("fixture-err").count(), 4096);
}

#[test]
fn native_capture_preserves_private_values_without_printing_them() {
    let directory = tempfile::tempdir().expect("create fixture root");
    let mut runtime = Runtime::with_inputs(
        directory.path().to_owned(),
        values(&[(MODE, "output"), ("REGISTRY_PASSWORD", PASSWORD)]),
    );
    let arguments = arguments();
    let borrowed: Vec<_> = arguments.iter().map(String::as_str).collect();
    let output = runtime
        .capture(&borrowed, Disclosure::Private)
        .expect("capture succeeds");
    assert!(output.contains(PASSWORD));
    assert!(!runtime.redact(&output).contains(PASSWORD));
    assert!(!format!("{runtime:?}").contains(PASSWORD));
}

#[test]
fn native_public_failure_redacts_stderr() {
    let directory = tempfile::tempdir().expect("create fixture root");
    let mut runtime = Runtime::with_inputs(
        directory.path().to_owned(),
        values(&[(MODE, "failure"), ("REGISTRY_PASSWORD", PASSWORD)]),
    );
    let arguments = arguments();
    let borrowed: Vec<_> = arguments.iter().map(String::as_str).collect();
    let error = runtime
        .capture(&borrowed, Disclosure::Public)
        .expect_err("fixture exits unsuccessfully")
        .to_string();
    assert!(error.contains("CLI command failed"));
    assert!(error.contains("[redacted]"));
    assert!(!error.contains(PASSWORD));
}

#[test]
fn native_private_failure_hides_unknown_credentials() {
    let directory = tempfile::tempdir().expect("create fixture root");
    let mut runtime =
        Runtime::with_inputs(directory.path().to_owned(), values(&[(MODE, "failure")]));
    let arguments = arguments();
    let borrowed: Vec<_> = arguments.iter().map(String::as_str).collect();
    let error = runtime
        .capture(&borrowed, Disclosure::Private)
        .expect_err("fixture exits unsuccessfully")
        .to_string();
    assert!(!error.contains(PASSWORD));
    assert!(!error.contains("synthetic failure"));
}

#[test]
fn cancellation_stops_and_reaps_the_started_child() {
    let cancelled = AtomicBool::new(false);
    let start = Instant::now();
    let error = process::run(command("wait"), &cancelled, |_, line| {
        if line.contains("fixture-ready") {
            cancelled.store(true, Ordering::SeqCst);
        }
        Ok(())
    })
    .expect_err("running child is cancelled")
    .to_string();
    assert!(error.contains("Cancelled"));
    assert!(start.elapsed() < Duration::from_secs(10));
}

#[test]
fn output_failure_also_stops_the_started_child() {
    let start = Instant::now();
    let error = process::run(command("wait"), &AtomicBool::new(false), |_, line| {
        anyhow::ensure!(!line.contains("fixture-ready"), "Simulated broken output.");
        Ok(())
    })
    .expect_err("output failure is propagated")
    .to_string();
    assert!(error.contains("broken output"));
    assert!(start.elapsed() < Duration::from_secs(10));
}

#[test]
fn cancellation_stops_hook_descendants() {
    let cancelled = AtomicBool::new(false);
    let mut listener = None;
    let start = Instant::now();
    let error = process::run_group(command("tree"), &cancelled, |_, line| {
        if let Some(connection) = connect_to_listener(line) {
            listener = Some(connection);
            cancelled.store(true, Ordering::SeqCst);
        }
        Ok(())
    })
    .expect_err("the provision group is cancelled")
    .to_string();
    assert!(error.contains("Cancelled"), "{error}");
    assert_listener_closed(listener.expect("hook descendant started"));
    assert!(start.elapsed() < Duration::from_secs(10));
}

#[test]
fn output_failure_stops_hook_descendants() {
    let mut listener = None;
    let start = Instant::now();
    let error = process::run_group(command("tree"), &AtomicBool::new(false), |_, line| {
        if let Some(connection) = connect_to_listener(line) {
            listener = Some(connection);
            anyhow::bail!("Simulated broken group output.");
        }
        Ok(())
    })
    .expect_err("the provision group stops on output failure")
    .to_string();
    assert!(error.contains("broken group output"), "{error}");
    assert_listener_closed(listener.expect("hook descendant started"));
    assert!(start.elapsed() < Duration::from_secs(10));
}

#[test]
fn grouped_commands_preserve_success_and_failure_status() {
    for (mode, success) in [("output", true), ("failure", false)] {
        let status = process::run_group(command(mode), &AtomicBool::new(false), |_, _| Ok(()))
            .expect("collect the grouped command's exit status");
        assert_eq!(status.success(), success);
    }
}

pub(in crate::provision) fn connect_to_listener(line: &str) -> Option<TcpStream> {
    let (_, address) = line.split_once("fixture-listener ")?;
    let address: SocketAddr = address
        .trim()
        .parse()
        .expect("fixture announces its address");
    let connection = TcpStream::connect_timeout(&address, Duration::from_secs(1))
        .expect("hook descendant is alive before cancellation");
    connection
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("bound the descendant shutdown check");
    Some(connection)
}

pub(in crate::provision) fn assert_listener_closed(mut connection: TcpStream) {
    match connection.read(&mut [0]) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted
            ) => {}
        result => panic!("hook descendant did not shut down: {result:?}"),
    }
}

#[test]
fn native_secret_prompt_refuses_redirected_input() {
    let mut stdout = String::new();
    let status = process::run(command("prompt"), &AtomicBool::new(false), |_, line| {
        stdout.push_str(line);
        Ok(())
    })
    .expect("run noninteractive prompt fixture");
    assert!(status.success());
    assert!(stdout.contains("fixture-prompt-refused"));
}

#[test]
#[ignore = "subprocess fixture invoked only by the native transport tests"]
fn subprocess_fixture() {
    match std::env::var(MODE)
        .expect("native test must explicitly select fixture mode")
        .as_str()
    {
        "output" => {
            println!("fixture-out {PASSWORD}");
            eprintln!("fixture-err {PASSWORD}");
        }
        "burst" => burst(),
        "failure" => {
            eprintln!("fixture-err {PASSWORD}");
            panic!("synthetic failure");
        }
        "wait" => {
            println!("fixture-ready");
            io::stdout().flush().expect("flush ready signal");
            std::thread::sleep(Duration::from_mins(1));
        }
        "prompt" => redirected_prompt(),
        "tree" => hook_tree(),
        "listener" => hook_listener(),
        _ => panic!("unknown fixture mode"),
    }
}

fn burst() {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    for _ in 0..4096 {
        writeln!(stdout, "fixture-out {PASSWORD}").expect("write fixture stdout");
        writeln!(stderr, "fixture-err {PASSWORD}").expect("write fixture stderr");
    }
}

fn redirected_prompt() {
    let mut runtime = Runtime::with_inputs(
        std::env::current_dir().expect("fixture has working directory"),
        values(&[]),
    );
    let error = runtime
        .prompt("REGISTRY_PASSWORD", "", Input::Secret)
        .expect_err("redirected input must not fall back to echoed input")
        .to_string();
    assert!(error.contains("interactive input"));
    println!("fixture-prompt-refused");
}

fn hook_tree() {
    let status = command("listener")
        .spawn()
        .expect("spawn a synthetic hook descendant")
        .wait()
        .expect("wait for the hook descendant");
    assert!(status.success());
}

fn hook_listener() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("listen on a local fixture port");
    println!(
        "fixture-listener {}",
        listener
            .local_addr()
            .expect("fixture listener has an address")
    );
    io::stdout()
        .flush()
        .expect("flush the descendant ready signal");
    std::thread::sleep(Duration::from_mins(1));
    drop(listener);
}
