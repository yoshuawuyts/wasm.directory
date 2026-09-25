//! Confirmed, infrastructure-only provisioning through the existing azd flow.

mod configuration;
mod environment;
mod host;
mod process;
mod redactor;
mod runtime;
mod store;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use anyhow::{Result, anyhow};

use host::{Disclosure, Host, confirm, required};
use runtime::Runtime;

type Values = BTreeMap<String, String>;

/// Prepare and confirm local azd configuration, then provision existing images.
pub(crate) fn run_provision(environment: Option<&str>) -> Result<()> {
    let mut runtime = Runtime::new(crate::workspace_root()?)?;
    run(&mut runtime, environment)
        .map_err(|error| anyhow!("{}", runtime.redact(&format!("{error:#}"))))
}

fn run(host: &mut impl Host, explicit: Option<&str>) -> Result<()> {
    let subscription = environment::check_tools(host)?;
    let selection = environment::select(host, explicit.filter(|name| !name.is_empty()))?;
    let saved = if selection.existing {
        store::read(host, &selection.name)?
    } else {
        Values::new()
    };
    let mut values = configuration::inputs(host, &selection.name, &saved)?;
    configuration::complete(host, &mut values, &saved, &subscription, selection.existing)?;
    show_target(host, &selection.name, &values)?;
    if values
        .iter()
        .any(|(key, value)| saved.get(key) != Some(value))
    {
        host.message(
            "Missing settings will be saved to .azure/<environment>/.env: \
             gitignored plaintext, NOT encrypted storage.",
        )?;
    }
    confirm(host, "Apply infrastructure to this target now?")?;
    host.prepare_apply()?;
    store::persist(host, &selection, &saved, &values)?;
    host.apply(&selection.name, &values)?;
    host.message(
        "azd provisioning completed. Review any deferred custom-domain warnings and verify \
         the website and API health before treating the rollout as healthy.",
    )
}

fn show_target(host: &mut impl Host, name: &str, values: &Values) -> Result<()> {
    host.message(&format!("\nProvisioning target: {name}"))?;
    for key in [
        "AZURE_SUBSCRIPTION_ID",
        "AZURE_LOCATION",
        "BACKEND_IMAGE",
        "FRONTEND_IMAGE",
    ] {
        host.message(&format!("  {key}: {}", required(values, key)?))?;
    }
    for key in configuration::DEPLOYMENT_OVERRIDES {
        let value = values
            .get(key)
            .filter(|value| !value.is_empty())
            .map_or("(Bicep default)", String::as_str);
        host.message(&format!("  {key}: {value}"))?;
    }
    host.message(
        "This applies Bicep and its provider/domain hooks using these images; it does not \
         build, publish, or release anything.\nAzure will validate deployment permissions \
         and image access. Existing database credentials will be re-applied.",
    )
}

fn capture(host: &mut impl Host, args: &[&str]) -> Result<String> {
    host.capture(args, Disclosure::Public)
}
