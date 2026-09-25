//! Literal dotenv imports with private temporary files and preservation checks.

use std::io::Write;

use anyhow::{Context, Result, ensure};

use super::Values;
use super::capture;
use super::environment::Selection;
use super::host::{Disclosure, Host, json, required};

pub(super) fn read(host: &mut impl Host, name: &str) -> Result<Values> {
    let values = json(
        host,
        &[
            "azd",
            "env",
            "get-values",
            "--environment",
            name,
            "--output",
            "json",
            "--no-prompt",
        ],
        Disclosure::Private,
        "azd settings",
    )
    .context(
        "Restore the selected environment's original .env file. \
         Raw diagnostics are suppressed because they can contain credentials.",
    )?;
    host.protect(&values);
    Ok(values)
}

pub(super) fn persist(
    host: &mut impl Host,
    selection: &Selection,
    saved: &Values,
    values: &Values,
) -> Result<()> {
    let current = if selection.existing {
        let current = read(host, &selection.name)?;
        ensure!(
            &current == saved,
            "The azd environment changed during setup. Rerun without overwriting it."
        );
        current
    } else {
        initialize(host, &selection.name, values)?
    };
    let additions: Values = values
        .iter()
        .filter(|(key, value)| current.get(*key) != Some(*value))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    if !additions.is_empty() {
        let keys = additions.keys().cloned().collect::<Vec<_>>().join(", ");
        host.message(&format!("Saving missing local settings: {keys}"))?;
        save_missing(host, &selection.name, &additions)?;
    }
    let stored = read(host, &selection.name)?;
    let mut expected = current;
    expected.extend(values.clone());
    ensure!(
        expected
            .iter()
            .all(|(key, value)| stored.get(key) == Some(value)),
        "azd did not preserve configuration exactly. Restore the original values \
         before retrying; provisioning was not started."
    );
    Ok(())
}

fn initialize(host: &mut impl Host, name: &str, values: &Values) -> Result<Values> {
    ensure!(
        !host.root().join(".azure").join(name).exists(),
        "Local environment configuration appeared during setup. Rerun without replacing it."
    );
    capture(
        host,
        &[
            "azd",
            "env",
            "new",
            name,
            "--subscription",
            required(values, "AZURE_SUBSCRIPTION_ID")?,
            "--location",
            required(values, "AZURE_LOCATION")?,
            "--no-prompt",
        ],
    )?;
    let saved = read(host, name)?;
    ensure!(
        values.iter().all(|(key, value)| {
            saved
                .get(key)
                .is_none_or(|saved| saved.is_empty() || saved == value)
        }),
        "azd initialized conflicting settings. Review the environment before retrying."
    );
    Ok(saved)
}

fn save_missing(host: &mut impl Host, name: &str, values: &Values) -> Result<()> {
    let directory = tempfile::tempdir().context("Cannot create a private import directory.")?;
    #[cfg(unix)]
    restrict_permissions(directory.path(), 0o700)?;
    let mut file = tempfile::NamedTempFile::new_in(directory.path())
        .context("Cannot create a private import file.")?;
    file.write_all(dotenv(values)?.as_bytes())
        .context("Cannot write the private import file.")?;
    file.flush()
        .context("Cannot flush the private import file.")?;
    let path = file
        .path()
        .to_str()
        .context("The private import path is not valid UTF-8.")?;
    host.capture(
        &[
            "azd",
            "env",
            "set",
            "--environment",
            name,
            "--file",
            path,
            "--no-prompt",
        ],
        Disclosure::Private,
    )?;
    let local = host.root().join(".azure").join(name).join(".env");
    ensure!(
        local.is_file(),
        "azd did not save a local .env; provisioning was not started."
    );
    #[cfg(unix)]
    restrict_permissions(&local, 0o600)?;
    Ok(())
}

#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .context("Cannot restrict permissions on a provisioning secret path.")
}

pub(super) fn validate_values(values: &Values) -> Result<()> {
    for (key, value) in values {
        ensure!(
            !value.contains('\0'),
            "{key} contains a NUL character and cannot be an environment value."
        );
        ensure!(
            !value.ends_with(['\\', '"']),
            "{key} ends with a backslash or double quote, which azd's dotenv writer \
             cannot reliably round-trip. Nothing will be saved or provisioned. Keep \
             the original credential and resolve azd storage compatibility before retrying."
        );
    }
    Ok(())
}

pub(super) fn dotenv(values: &Values) -> Result<String> {
    validate_values(values)?;
    let mut output = String::new();
    for (key, value) in values {
        ensure!(
            key.starts_with(|c: char| c.is_ascii_uppercase())
                && key
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'),
            "Cannot store an invalid environment key."
        );
        output.push_str(key);
        output.push_str("=\"");
        escape_value(&mut output, value);
        output.push_str("\"\n");
    }
    Ok(output)
}

fn escape_value(output: &mut String, value: &str) {
    for c in value.chars() {
        match c {
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\\' | '"' | '!' | '$' | '`' => {
                output.push('\\');
                output.push(c);
            }
            _ => output.push(c),
        }
    }
}
