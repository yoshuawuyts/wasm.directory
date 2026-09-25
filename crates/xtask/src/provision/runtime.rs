//! Native CLI and terminal implementation; all subprocess output is filtered.

use std::fmt;
use std::io::{self, IsTerminal as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, anyhow, ensure};
use console::Term;

use super::Values;
use super::host::{Disclosure, Host, Input};
use super::process::{self, Stream};
use super::redactor::Redactor;

pub(super) struct Runtime {
    root: PathBuf,
    inputs: Values,
    redactor: Redactor,
    cancelled: Arc<AtomicBool>,
}

impl fmt::Debug for Runtime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Runtime")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl Runtime {
    pub(super) fn new(root: PathBuf) -> Result<Self> {
        let inputs: Values = std::env::vars_os()
            .map(|(key, value)| {
                let key = key
                    .into_string()
                    .map_err(|_| anyhow!("A process environment key is not valid UTF-8."))?;
                let value = value
                    .into_string()
                    .map_err(|_| anyhow!("Process input {key} is not valid UTF-8."))?;
                Ok((key, value))
            })
            .collect::<Result<_>>()?;
        Ok(Self::with_inputs(root, inputs))
    }

    pub(super) fn with_inputs(root: PathBuf, inputs: Values) -> Self {
        let mut redactor = Redactor::default();
        redactor.protect(&inputs);
        Self {
            root,
            inputs,
            redactor,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(super) fn redact(&self, message: &str) -> String {
        self.redactor.redact(message)
    }

    pub(super) fn provision_command(&self, environment: &str, values: &Values) -> Result<Command> {
        let mut command = process::command(&[
            "azd",
            "provision",
            "--environment",
            environment,
            "--no-prompt",
        ])?;
        command
            .current_dir(&self.root)
            .envs(&self.inputs)
            .envs(values);
        Ok(command)
    }
}

impl Host for Runtime {
    fn root(&self) -> &Path {
        &self.root
    }

    fn inputs(&self) -> &Values {
        &self.inputs
    }

    fn capture(&mut self, args: &[&str], disclosure: Disclosure) -> Result<String> {
        let mut command = process::command(args)?;
        command.current_dir(&self.root).envs(&self.inputs);
        let mut stdout = String::new();
        let mut stderr = String::new();
        let status = process::run(command, &self.cancelled, |stream, text| {
            match stream {
                Stream::Stdout => stdout.push_str(text),
                Stream::Stderr => stderr.push_str(text),
            }
            Ok(())
        })?;
        let detail = match disclosure {
            Disclosure::Private => String::new(),
            Disclosure::Public => self.redact(stderr.trim()),
        };
        ensure!(status.success(), "CLI command failed ({status}). {detail}");
        Ok(stdout)
    }

    fn prompt(&mut self, label: &str, default: &str, input: Input) -> Result<String> {
        process::check_cancelled(&self.cancelled)?;
        let term = Term::stderr();
        ensure!(
            io::stdin().is_terminal() && term.is_term(),
            "{label} requires interactive input. Restore the existing azd configuration, \
             supply missing settings through environment variables, or rerun in a terminal."
        );
        let suffix = if default.is_empty() {
            String::new()
        } else {
            format!(" [{default}]")
        };
        term.write_str(&self.redact(&format!("{label}{suffix}: ")))?;
        match input {
            Input::Secret => term.read_secure_line().context(
                "Cannot read a secret without echo; input was cancelled or the terminal failed.",
            ),
            Input::Plain => {
                let line = term
                    .read_line()
                    .context("Terminal input was cancelled or failed.")?;
                let trimmed = line.trim();
                Ok(if trimmed.is_empty() {
                    default.to_owned()
                } else {
                    trimmed.to_owned()
                })
            }
        }
    }

    fn message(&mut self, message: &str) -> Result<()> {
        writeln!(io::stdout().lock(), "{}", self.redact(message))
            .context("Cannot write provisioning status.")
    }

    fn protect(&mut self, values: &Values) {
        self.redactor.protect(values);
    }

    fn prepare_apply(&mut self) -> Result<()> {
        // Install only after terminal prompts, before any secret file is created.
        let cancelled = Arc::clone(&self.cancelled);
        ctrlc::set_handler(move || cancelled.store(true, Ordering::SeqCst))
            .context("Cannot install the provisioning cancellation handler.")
    }

    fn apply(&mut self, environment: &str, values: &Values) -> Result<()> {
        let command = self.provision_command(environment, values)?;
        let status = process::run(command, &self.cancelled, |_, text| {
            let mut stdout = io::stdout().lock();
            stdout.write_all(self.redact(text).as_bytes())?;
            stdout.flush()?;
            Ok(())
        })?;
        ensure!(
            status.success(),
            "azd provision failed ({status}); resources may be partially updated."
        );
        Ok(())
    }
}
