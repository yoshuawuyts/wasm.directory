//! Authentication and explicit azd environment selection.

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

use super::capture;
use super::host::{Disclosure, Host, Input, confirm, json};

#[derive(Debug)]
pub(super) struct Selection {
    pub(super) name: String,
    pub(super) existing: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Description {
    name: String,
    is_default: bool,
    has_local: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct Account {
    id: String,
    state: String,
}

pub(super) fn check_tools(host: &mut impl Host) -> Result<String> {
    let help = capture(host, &["azd", "env", "set", "--help"])?;
    ensure!(
        help.contains("--file"),
        "Update azd: provisioning requires 'azd env set --file'."
    );
    let account: Account = json(
        host,
        &["az", "account", "show", "--output", "json"],
        Disclosure::Public,
        "Azure account",
    )
    .context("Sign in explicitly with 'az login'.")?;
    ensure!(
        !account.id.is_empty() && account.state == "Enabled",
        "az did not report an enabled subscription. Run 'az login'."
    );
    capture(
        host,
        &[
            "az",
            "account",
            "get-access-token",
            "--query",
            "expiresOn",
            "--output",
            "tsv",
        ],
    )
    .context("Renew the Azure CLI session with 'az login'.")?;
    capture(host, &["azd", "auth", "login", "--check-status"])
        .context("Sign in explicitly with 'azd auth login'.")?;
    Ok(account.id)
}

pub(super) fn select(host: &mut impl Host, explicit: Option<&str>) -> Result<Selection> {
    let environments: Vec<Description> = json(
        host,
        &["azd", "env", "list", "--output", "json", "--no-prompt"],
        Disclosure::Public,
        "environment list",
    )?;
    let name = select_name(host, &environments, explicit)?;
    ensure!(
        valid_name(&name),
        "Use a 1-64 character azd environment name, without path separators."
    );
    let existing = environments.iter().find(|env| env.name == name);
    match existing {
        Some(env) => ensure!(
            env.has_local != Some(false),
            "The selected environment is remote-only. Restore its local configuration \
             explicitly before provisioning."
        ),
        None => confirm_new(host, &environments, &name)?,
    }
    Ok(Selection {
        name,
        existing: existing.is_some(),
    })
}

fn select_name(
    host: &mut impl Host,
    environments: &[Description],
    explicit: Option<&str>,
) -> Result<String> {
    if let Some(name) = explicit {
        return Ok(name.to_owned());
    }
    if let Some(name) = host.inputs().get("AZURE_ENV_NAME")
        && !name.is_empty()
    {
        return Ok(name.clone());
    }
    let defaults: Vec<_> = environments.iter().filter(|env| env.is_default).collect();
    if let [default] = defaults.as_slice() {
        return Ok(default.name.clone());
    }
    if !environments.is_empty() {
        let names: Vec<_> = environments.iter().map(|env| env.name.as_str()).collect();
        host.message(&format!("Available azd environments: {}", names.join(", ")))?;
    }
    host.prompt(
        "azd environment name (restore existing configuration when possible)",
        "",
        Input::Plain,
    )
}

fn confirm_new(host: &mut impl Host, environments: &[Description], name: &str) -> Result<()> {
    if environments
        .iter()
        .any(|env| env.name.eq_ignore_ascii_case(name))
    {
        bail!("Use the existing environment's exact name; its case will not be changed.");
    }
    ensure!(
        !host.root().join(".azure").join(name).exists(),
        "Local configuration already exists but azd did not list this environment. \
         Restore it explicitly; the helper will not reinitialize that directory."
    );
    confirm(
        host,
        &format!(
            "Create local azd configuration for '{name}'? For an existing deployment, \
             recover its original settings and password, not a replacement password."
        ),
    )
}

fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name.starts_with(|c: char| c.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}
