//! Concurrent pipe draining and cancellation of only the child we started.

use std::io::{self, BufReader, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail, ensure};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stream {
    Stdout,
    Stderr,
}

type Message = (Stream, io::Result<Vec<u8>>);

pub(super) fn run(
    mut command: Command,
    cancelled: &AtomicBool,
    mut emit: impl FnMut(Stream, &str) -> Result<()>,
) -> Result<ExitStatus> {
    check_cancelled(cancelled)?;
    let program = command.get_program().to_string_lossy().into_owned();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| {
            format!("Cannot start {program}; ensure the required CLI is installed.")
        })?;
    let (sender, receiver) = mpsc::channel();
    let result = start_readers(&mut child, sender)
        .and_then(|()| drain(&mut child, &receiver, cancelled, &mut emit));
    if result.is_err() {
        stop_child(&mut child)?;
    }
    result
}

fn start_readers(child: &mut Child, sender: Sender<Message>) -> Result<()> {
    reader(
        child.stdout.take().expect("stdout was piped"),
        Stream::Stdout,
        sender.clone(),
    )
    .context("Cannot start the CLI stdout reader.")?;
    reader(
        child.stderr.take().expect("stderr was piped"),
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
    child: &mut Child,
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

fn wait(child: &mut Child, cancelled: &AtomicBool) -> Result<ExitStatus> {
    loop {
        check_cancelled(cancelled)?;
        if let Some(status) = child.try_wait().context("Cannot poll the CLI child.")? {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn stop_child(child: &mut Child) -> Result<()> {
    if child
        .try_wait()
        .context("Cannot check the CLI child.")?
        .is_none()
    {
        match child.kill() {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::InvalidInput => {}
            Err(error) => {
                bail!(
                    "Cannot stop CLI child {} after interruption: {error}",
                    child.id()
                );
            }
        }
    }
    child.wait().context("Cannot reap the CLI child.")?;
    Ok(())
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
