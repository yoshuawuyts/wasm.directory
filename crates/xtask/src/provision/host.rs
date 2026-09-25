//! Side-effect boundary shared by the native runtime and offline tests.

use std::path::Path;

use anyhow::{Result, anyhow, bail};
use serde::de::DeserializeOwned;

use super::Values;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Disclosure {
    Public,
    Private,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Input {
    Plain,
    Secret,
}

pub(super) trait Host {
    fn root(&self) -> &Path;
    fn inputs(&self) -> &Values;
    fn capture(&mut self, args: &[&str], disclosure: Disclosure) -> Result<String>;
    fn prompt(&mut self, label: &str, default: &str, input: Input) -> Result<String>;
    fn message(&mut self, message: &str) -> Result<()>;
    fn protect(&mut self, values: &Values);
    fn prepare_apply(&mut self) -> Result<()>;
    fn apply(&mut self, environment: &str, values: &Values) -> Result<()>;
}

pub(super) fn json<T: DeserializeOwned>(
    host: &mut impl Host,
    args: &[&str],
    disclosure: Disclosure,
    description: &str,
) -> Result<T> {
    let output = host.capture(args, disclosure)?;
    serde_json::from_str(&output).map_err(|_| {
        anyhow!("Invalid {description} returned by azd/az; its contents were not printed.")
    })
}

pub(super) fn confirm(host: &mut impl Host, message: &str) -> Result<()> {
    let answer = host.prompt(&format!("{message} [y/N]"), "", Input::Plain)?;
    if !matches!(answer.to_ascii_lowercase().as_str(), "y" | "yes") {
        bail!("Cancelled; provisioning was not started.");
    }
    Ok(())
}

pub(super) fn required<'a>(values: &'a Values, key: &str) -> Result<&'a str> {
    values
        .get(key)
        .filter(|value| !value.is_empty())
        .map(String::as_str)
        .ok_or_else(|| anyhow!("{key} is required; no value was generated."))
}

pub(super) fn has_value(values: &Values, key: &str) -> bool {
    values.get(key).is_some_and(|value| !value.is_empty())
}
