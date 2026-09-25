//! Reuse existing configuration and guide only genuinely missing inputs.

use std::collections::BTreeSet;

use anyhow::{Context, Result, bail, ensure};

use super::Values;
use super::host::{Host, Input, confirm, has_value, required};
use super::store;

pub(super) fn inputs(host: &mut impl Host, name: &str, saved: &Values) -> Result<Values> {
    let source = std::fs::read_to_string(host.root().join("infra/main.bicepparam"))
        .context("Cannot read infra/main.bicepparam.")?;
    let mut keys: BTreeSet<_> = source
        .split("readEnvironmentVariable(")
        .skip(1)
        .filter_map(|part| part.trim_start().strip_prefix('\'')?.split_once('\''))
        .map(|(name, _)| name.to_owned())
        .collect();
    ensure!(
        keys.contains("POSTGRES_ADMIN_PASSWORD"),
        "Cannot identify the deployment's environment parameters."
    );
    keys.extend(["AZURE_SUBSCRIPTION_ID".into(), "AZURE_TENANT_ID".into()]);
    keys.extend(saved.keys().cloned());
    let mut values = saved.clone();
    let mut conflicts = BTreeSet::new();
    for key in keys {
        if let Some(incoming) = host.inputs().get(&key) {
            merge_input(&mut values, &mut conflicts, key, incoming);
        }
    }
    if let Some(value) = values.get("AZURE_ENV_NAME")
        && value != name
    {
        conflicts.insert("AZURE_ENV_NAME".into());
    }
    ensure!(
        conflicts.is_empty(),
        "Process inputs conflict with saved/selected settings: {}. \
         Unset conflicting inputs or deliberately update the azd environment first. \
         Existing values were not overwritten.",
        conflicts.into_iter().collect::<Vec<_>>().join(", ")
    );
    values.insert("AZURE_ENV_NAME".into(), name.into());
    host.protect(&values);
    Ok(values)
}

fn merge_input(values: &mut Values, conflicts: &mut BTreeSet<String>, key: String, incoming: &str) {
    match values.get(&key).filter(|value| !value.is_empty()) {
        Some(saved) if saved != incoming => {
            conflicts.insert(key);
        }
        None if !incoming.is_empty() => {
            values.insert(key, incoming.into());
        }
        _ => {}
    }
}

pub(super) fn complete(
    host: &mut impl Host,
    values: &mut Values,
    saved: &Values,
    subscription: &str,
    existing: bool,
) -> Result<()> {
    let subscription_default = if existing { "" } else { subscription };
    require_setting(host, values, "AZURE_SUBSCRIPTION_ID", subscription_default)?;
    ensure!(
        required(values, "AZURE_SUBSCRIPTION_ID")?.eq_ignore_ascii_case(subscription),
        "The az CLI subscription differs from this deployment. Explicitly run \
         'az account set --subscription <the deployment subscription>' and retry; \
         the helper will not switch subscriptions for the provisioning hooks."
    );
    require_setting(host, values, "AZURE_LOCATION", "")?;
    if !existing {
        host.message(
            "For an existing deployment, restore its exact region, resource group, \
             database settings, and domain. Changing them can target new resources.",
        )?;
        optional_settings(host, values)?;
    }
    for key in ["BACKEND_IMAGE", "FRONTEND_IMAGE", "POSTGRES_ADMIN_PASSWORD"] {
        require_setting(host, values, key, "")?;
    }
    let missing_images = ["BACKEND_IMAGE", "FRONTEND_IMAGE"]
        .iter()
        .any(|key| !has_value(saved, key));
    registry_settings(host, values, missing_images)?;
    store::validate_values(values)?;
    for key in ["POSTGRES_ADMIN_LOGIN", "POSTGRES_DB"] {
        ensure!(
            values.get(key).is_none_or(|value| !value.is_empty()),
            "{key} is explicitly empty. Restore its original value, or remove the setting \
             deliberately to use the Bicep default."
        );
    }
    check_images(host, values)
}

fn require_setting(
    host: &mut impl Host,
    values: &mut Values,
    key: &str,
    default: &str,
) -> Result<()> {
    if has_value(values, key) {
        return Ok(());
    }
    let (label, input) = match key {
        "AZURE_SUBSCRIPTION_ID" if default.is_empty() => (
            format!("{key} (recover this deployment's original subscription ID)"),
            Input::Plain,
        ),
        "POSTGRES_ADMIN_PASSWORD" => (
            format!("{key} (original password for an existing deployment; input hidden)"),
            Input::Secret,
        ),
        "REGISTRY_PASSWORD" => (format!("{key} (input hidden)"), Input::Secret),
        _ => (key.to_owned(), Input::Plain),
    };
    let value = host.prompt(&label, default, input)?;
    ensure!(
        !value.is_empty(),
        "{key} is required; no value was generated."
    );
    values.insert(key.into(), value);
    host.protect(values);
    Ok(())
}

fn optional_settings(host: &mut impl Host, values: &mut Values) -> Result<()> {
    for key in [
        "AZURE_RESOURCE_GROUP",
        "POSTGRES_ADMIN_LOGIN",
        "POSTGRES_DB",
        "CUSTOM_DOMAIN_NAME",
    ] {
        if values.contains_key(key) {
            continue;
        }
        let value = host.prompt(
            &format!("{key} (optional; blank uses the existing Bicep default)"),
            "",
            Input::Plain,
        )?;
        if !value.is_empty() {
            values.insert(key.into(), value);
        }
    }
    Ok(())
}

fn registry_settings(
    host: &mut impl Host,
    values: &mut Values,
    ask_visibility: bool,
) -> Result<()> {
    let credentials =
        has_value(values, "REGISTRY_USERNAME") || has_value(values, "REGISTRY_PASSWORD");
    if !has_value(values, "REGISTRY_SERVER") && credentials {
        require_setting(host, values, "REGISTRY_SERVER", "")?;
    }
    if !has_value(values, "REGISTRY_SERVER") && ask_visibility {
        let answer = host.prompt(
            "Are both selected images publicly pullable without credentials? [y/n]",
            "",
            Input::Plain,
        )?;
        match answer.to_ascii_lowercase().as_str() {
            "y" | "yes" => {}
            "n" | "no" => require_setting(host, values, "REGISTRY_SERVER", "")?,
            _ => bail!("Image visibility must be confirmed; provisioning was not started."),
        }
    }
    if !has_value(values, "REGISTRY_SERVER") {
        return Ok(());
    }
    let server = required(values, "REGISTRY_SERVER")?;
    ensure!(
        valid_server(server),
        "REGISTRY_SERVER must be a hostname, optionally with a port."
    );
    let prefix = format!("{}/", server.to_ascii_lowercase());
    ensure!(
        ["BACKEND_IMAGE", "FRONTEND_IMAGE"].iter().any(|key| {
            values
                .get(*key)
                .is_some_and(|image| image.to_ascii_lowercase().starts_with(&prefix))
        }),
        "REGISTRY_SERVER does not match either selected image's registry."
    );
    for key in ["REGISTRY_USERNAME", "REGISTRY_PASSWORD"] {
        require_setting(host, values, key, "")?;
    }
    Ok(())
}

fn valid_server(server: &str) -> bool {
    let (host, port) = match server.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (server, None),
    };
    !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
        && port.is_none_or(|port| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()))
}

pub(super) fn check_images(host: &mut impl Host, values: &Values) -> Result<()> {
    let mut mutable = Vec::new();
    for key in ["BACKEND_IMAGE", "FRONTEND_IMAGE"] {
        let image = required(values, key)?;
        ensure!(
            valid_image(image),
            "{key} must be a container image reference, not a URL or placeholder."
        );
        let repository = image
            .split(['@', ':'])
            .next()
            .expect("split always has an element");
        ensure!(
            repository != "mcr.microsoft.com/azuredocs/containerapps-helloworld",
            "{key} is the Bicep demo image; supply the actual application image."
        );
        let last_segment = image
            .rsplit('/')
            .next()
            .expect("split always has an element");
        if !image.contains('@') && (!last_segment.contains(':') || image.ends_with(":latest")) {
            mutable.push(key);
        }
    }
    if !mutable.is_empty() {
        confirm(
            host,
            &format!(
                "Keep explicitly configured latest/untagged images for {}? \
                 A new revision may pull different code; a version tag or digest is safer.",
                mutable.join(", ")
            ),
        )?;
    }
    Ok(())
}

fn valid_image(image: &str) -> bool {
    let (reference, digest) = match image.split_once('@') {
        Some((reference, digest)) => (reference, Some(digest)),
        None => (image, None),
    };
    reference.starts_with(|c: char| c.is_ascii_alphanumeric())
        && !reference.contains("://")
        && reference
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '/' | '-'))
        && digest.is_none_or(|digest| {
            digest
                .strip_prefix("sha256:")
                .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        })
}
