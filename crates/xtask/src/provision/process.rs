//! Concurrent pipe draining and cancellation of the processes we started.

use std::io::{self, BufReader, Read};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, ensure};
use process_wrap::std::{ChildWrapper, CommandWrap};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stream {
    Stdout,
    Stderr,
}

type Message = (Stream, io::Result<Vec<u8>>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scope {
    Child,
    Tree,
}

pub(super) fn run(
    command: Command,
    cancelled: &AtomicBool,
    emit: impl FnMut(Stream, &str) -> Result<()>,
) -> Result<ExitStatus> {
    run_scoped(command, Scope::Child, cancelled, emit)
}

pub(super) fn run_group(
    command: Command,
    cancelled: &AtomicBool,
    emit: impl FnMut(Stream, &str) -> Result<()>,
) -> Result<ExitStatus> {
    run_scoped(command, Scope::Tree, cancelled, emit)
}

fn run_scoped(
    command: Command,
    scope: Scope,
    cancelled: &AtomicBool,
    mut emit: impl FnMut(Stream, &str) -> Result<()>,
) -> Result<ExitStatus> {
    check_cancelled(cancelled)?;
    let mut child = spawn(command, scope)?;
    let (sender, receiver) = mpsc::channel();
    let result = start_readers(child.as_mut(), sender)
        .and_then(|()| drain(child.as_mut(), &receiver, cancelled, &mut emit));
    if result.is_err() {
        stop_child(child.as_mut(), scope)?;
    }
    result
}

fn spawn(mut command: Command, scope: Scope) -> Result<Box<dyn ChildWrapper>> {
    let program = command.get_program().to_string_lossy().into_owned();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut command = CommandWrap::from(command);
    if scope == Scope::Tree {
        #[cfg(unix)]
        command.wrap(process_wrap::std::ProcessGroup::leader());
        #[cfg(windows)]
        command.wrap(process_wrap::std::JobObject);
        #[cfg(not(any(unix, windows)))]
        anyhow::bail!("Provisioning requires process-group or job-object support.");
    }
    command
        .spawn()
        .with_context(|| format!("Cannot start {program}; ensure the required CLI is installed."))
}

fn start_readers(child: &mut dyn ChildWrapper, sender: Sender<Message>) -> Result<()> {
    reader(
        child.stdout().take().expect("stdout was piped"),
        Stream::Stdout,
        sender.clone(),
    )
    .context("Cannot start the CLI stdout reader.")?;
    reader(
        child.stderr().take().expect("stderr was piped"),
        Stream::Stderr,
        sender,
    )
    .context("Cannot start the CLI stderr reader.")
}

fn reader(
    pipe: impl Read + Send + 'static,
    stream: Stream,
    sender: Sender<Message>,
) -> io::Result<()> {
    thread::Builder::new().spawn(move || {
        let mut reader = BufReader::new(pipe);
        while read_chunk(&mut reader, stream, &sender) {}
    })?;
    Ok(())
}

fn read_chunk(reader: &mut impl io::BufRead, stream: Stream, sender: &Sender<Message>) -> bool {
    let mut bytes = Vec::new();
    match reader.read_until(b'\n', &mut bytes) {
        Ok(0) => false,
        Ok(_) => sender.send((stream, Ok(bytes))).is_ok(),
        Err(error) => {
            // The receiver may already have handled cancellation.
            drop(sender.send((stream, Err(error))));
            false
        }
    }
}

fn drain(
    child: &mut dyn ChildWrapper,
    receiver: &mpsc::Receiver<Message>,
    cancelled: &AtomicBool,
    emit: &mut impl FnMut(Stream, &str) -> Result<()>,
) -> Result<ExitStatus> {
    loop {
        check_cancelled(cancelled)?;
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok((stream, bytes)) => emit(stream, &String::from_utf8_lossy(&bytes?))?,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    wait(child, cancelled)
}

fn wait(child: &mut dyn ChildWrapper, cancelled: &AtomicBool) -> Result<ExitStatus> {
    loop {
        check_cancelled(cancelled)?;
        if let Some(status) = child.try_wait().context("Cannot poll the CLI child.")? {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn stop_child(child: &mut dyn ChildWrapper, scope: Scope) -> Result<()> {
    if scope == Scope::Child
        && child
            .try_wait()
            .context("Cannot check the CLI child.")?
            .is_some()
    {
        return Ok(());
    }
    // A group can still contain live hooks after its leader has exited.
    child.kill().with_context(|| {
        format!(
            "Cannot stop and reap CLI process {} ({scope:?}) after interruption.",
            child.id()
        )
    })
}

pub(super) fn check_cancelled(cancelled: &AtomicBool) -> Result<()> {
    ensure!(
        !cancelled.load(Ordering::SeqCst),
        "Cancelled. Azure resources may be partially updated; cancellation is not rollback."
    );
    Ok(())
}

pub(super) fn command(args: &[&str]) -> Result<Command> {
    let (program, arguments) = args
        .split_first()
        .ok_or_else(|| anyhow!("Cannot run an empty command."))?;
    let mut command = Command::new(program);
    command.args(arguments);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead as _, BufReader};

    use super::{Scope, spawn, stop_child};
    use crate::provision::tests::native;

    #[test]
    fn cancellation_stops_the_group_even_after_its_leader_exits() {
        let mut child = spawn(native::command("tree"), Scope::Tree).expect("start a hook group");
        let stdout = child.stdout().take().expect("fixture stdout is piped");
        let connection = BufReader::new(stdout)
            .lines()
            .find_map(|line| native::connect_to_listener(&line.expect("read fixture output")))
            .expect("the hook descendant started");
        let leader = child.inner_mut();
        leader.kill().expect("terminate only the group leader");
        assert!(!leader.wait().expect("reap the group leader").success());
        stop_child(child.as_mut(), Scope::Tree).expect("stop the orphaned hook group");
        native::assert_listener_closed(connection);
    }

    #[test]
    fn an_exited_ungrouped_child_is_not_signalled_again() {
        let mut child = spawn(native::command("output"), Scope::Child).expect("start fixture");
        assert!(child.wait().expect("fixture exits normally").success());
        stop_child(child.as_mut(), Scope::Child).expect("already reaped ungrouped child");
    }
}
