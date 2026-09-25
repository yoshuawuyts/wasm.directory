"""Provision existing images using the repository's azd/Bicep configuration."""

import argparse
from pathlib import Path
import re
import shlex
import shutil
import sys

from provision_support import Commands, ProvisionError, confirm, prompt, validate_values


def check_tools(commands: Commands) -> dict[str, str]:
    """Check supported tools and existing authentication without changing it."""
    for tool, guide in (
        ("az", "https://aka.ms/installazurecli"),
        ("azd", "https://aka.ms/install-azd"),
    ):
        if not shutil.which(tool):
            raise ProvisionError(f"{tool} is not installed. Install it from {guide}.")
    if "--file" not in commands.capture("azd", "env", "set", "--help"):
        raise ProvisionError("Update azd: provisioning requires 'azd env set --file'.")
    try:
        account = commands.json("az", "account", "show", "--output", "json")
    except ProvisionError as error:
        raise ProvisionError(f"{error}\nSign in explicitly with 'az login'.") from None
    if (
        not isinstance(account, dict)
        or not isinstance(account.get("id"), str)
        or not account["id"]
        or account.get("state") != "Enabled"
    ):
        raise ProvisionError("az did not report an enabled subscription. Run 'az login'.")
    try:
        commands.capture("az", "account", "get-access-token", "--query", "expiresOn", "--output", "tsv")
    except ProvisionError as error:
        raise ProvisionError(f"{error}\nRenew the Azure CLI session with 'az login'.") from None
    try:
        commands.capture("azd", "auth", "login", "--check-status")
    except ProvisionError as error:
        raise ProvisionError(f"{error}\nSign in explicitly with 'azd auth login'.") from None
    return {"id": account["id"]}


def select_environment(commands: Commands, explicit: str) -> tuple[str, bool]:
    """Select an existing environment, or obtain consent for local setup."""
    data = commands.json("azd", "env", "list", "--output", "json", "--no-prompt")
    if not isinstance(data, list) or not all(
        isinstance(item, dict)
        and isinstance(item.get("Name"), str)
        and isinstance(item.get("IsDefault"), bool)
        for item in data
    ):
        raise ProvisionError("azd returned an invalid environment list.")
    names = [item["Name"] for item in data]
    defaults = [item["Name"] for item in data if item["IsDefault"]]
    name = explicit or commands.environment.get("AZURE_ENV_NAME", "")
    if not name and len(defaults) == 1:
        name = defaults[0]
    if not name:
        if names:
            print("Available azd environments: " + ", ".join(names))
        name = prompt("azd environment name (restore existing configuration when possible)")
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,63}", name):
        raise ProvisionError("Use a 1-64 character azd environment name, without path separators.")
    existing = name in names
    if existing:
        entry = next(item for item in data if item["Name"] == name)
        if entry.get("HasLocal") is False:
            raise ProvisionError(
                "The selected environment is remote-only. Restore its local configuration "
                "explicitly before provisioning; this helper does not change remote state stores."
            )
    else:
        if any(item.casefold() == name.casefold() for item in names):
            raise ProvisionError("Use the existing environment's exact name; its case will not be changed.")
        if (commands.root / ".azure" / name).exists():
            raise ProvisionError(
                "Local configuration already exists but azd did not list this environment. "
                "Restore it explicitly; the helper will not reinitialize that directory."
            )
        confirm(
            f"Create local azd configuration for '{name}'? For an existing deployment, "
            "recover its original settings and password, not a replacement password."
        )
    return name, existing


def configured_values(
    commands: Commands, name: str, saved: dict[str, str]
) -> dict[str, str]:
    """Preserve saved settings; allow declared process inputs to fill gaps."""
    source = (commands.root / "infra" / "main.bicepparam").read_text(encoding="utf-8")
    keys = set(re.findall(r"readEnvironmentVariable\(\s*'([A-Z][A-Z0-9_]*)'", source))
    if "POSTGRES_ADMIN_PASSWORD" not in keys:
        raise ProvisionError("Cannot identify the deployment's environment parameters.")
    keys.update({"AZURE_SUBSCRIPTION_ID", "AZURE_TENANT_ID"})
    keys.update(saved)
    values = dict(saved)
    conflicts = []
    for key in sorted(keys):
        incoming = commands.environment.get(key)
        if incoming is None:
            continue
        if values.get(key) and incoming != values[key]:
            conflicts.append(key)
        elif incoming:
            values[key] = incoming
    if values.get("AZURE_ENV_NAME", name) != name:
        conflicts.append("AZURE_ENV_NAME")
    if conflicts:
        raise ProvisionError(
            "Process inputs conflict with saved/selected settings: "
            + ", ".join(sorted(set(conflicts)))
            + ". Unset conflicting inputs or deliberately update the azd environment first. "
            "Existing values were not overwritten."
        )
    values["AZURE_ENV_NAME"] = name
    commands.protect(values)
    return values


def require_setting(
    commands: Commands, values: dict[str, str], key: str, *, default: str = ""
) -> None:
    """Obtain a genuinely missing setting, preserving secret whitespace."""
    if values.get(key):
        return
    secret = key in {"POSTGRES_ADMIN_PASSWORD", "REGISTRY_PASSWORD"}
    label = key
    if key == "POSTGRES_ADMIN_PASSWORD":
        label += " (original password for an existing deployment; input hidden)"
    elif secret:
        label += " (input hidden)"
    value = prompt(label, default=default, secret=secret)
    if not value:
        raise ProvisionError(f"{key} is required; no value was generated.")
    values[key] = value
    commands.protect(values)


def complete_settings(
    commands: Commands, values: dict[str, str], saved: dict[str, str],
    account: dict[str, str], *, existing: bool,
) -> None:
    """Guide missing configuration without inventing a deployment policy."""
    require_setting(commands, values, "AZURE_SUBSCRIPTION_ID", default=account["id"])
    if values["AZURE_SUBSCRIPTION_ID"].lower() != account["id"].lower():
        raise ProvisionError(
            "The az CLI subscription differs from this deployment. Explicitly run "
            f"'az account set --subscription {shlex.quote(values['AZURE_SUBSCRIPTION_ID'])}' "
            "and retry; the helper will not switch subscriptions for the provisioning hooks."
        )
    require_setting(commands, values, "AZURE_LOCATION")
    if not existing:
        print(
            "For an existing deployment, restore its exact region, resource group, "
            "database settings, and domain. Changing them can target new resources."
        )
        optional_settings(values)
    for key in ("BACKEND_IMAGE", "FRONTEND_IMAGE", "POSTGRES_ADMIN_PASSWORD"):
        require_setting(commands, values, key)
    images_changed = any(not saved.get(key) for key in ("BACKEND_IMAGE", "FRONTEND_IMAGE"))
    registry_settings(commands, values, ask_visibility=images_changed)
    validate_values(values)
    for key in ("POSTGRES_ADMIN_LOGIN", "POSTGRES_DB"):
        if key in values and not values[key]:
            raise ProvisionError(
                f"{key} is explicitly empty. Restore its original value, or remove "
                "the setting deliberately to use the Bicep default."
            )
    check_images(values)


def optional_settings(values: dict[str, str]) -> None:
    """Let a fresh local environment restore optional deployment settings."""
    for key in (
        "AZURE_RESOURCE_GROUP", "POSTGRES_ADMIN_LOGIN", "POSTGRES_DB", "CUSTOM_DOMAIN_NAME",
    ):
        if key not in values:
            value = prompt(f"{key} (optional; blank uses the existing Bicep default)")
            if value:
                values[key] = value


def registry_settings(
    commands: Commands, values: dict[str, str], *, ask_visibility: bool
) -> None:
    """Require pull credentials only for explicitly selected private access."""
    has_credentials = bool(values.get("REGISTRY_USERNAME") or values.get("REGISTRY_PASSWORD"))
    if not values.get("REGISTRY_SERVER") and has_credentials:
        require_setting(commands, values, "REGISTRY_SERVER")
    if not values.get("REGISTRY_SERVER") and ask_visibility:
        answer = prompt("Are both selected images publicly pullable without credentials? [y/n]")
        if answer.lower() not in {"y", "yes", "n", "no"}:
            raise ProvisionError("Image visibility must be confirmed; provisioning was not started.")
        if answer.lower() in {"n", "no"}:
            require_setting(commands, values, "REGISTRY_SERVER")
    if not values.get("REGISTRY_SERVER"):
        return
    server = values["REGISTRY_SERVER"]
    if not re.fullmatch(r"[A-Za-z0-9.-]+(?::[0-9]+)?", server):
        raise ProvisionError("REGISTRY_SERVER must be a hostname, optionally with a port.")
    if not any(
        values[key].lower().startswith(server.lower() + "/")
        for key in ("BACKEND_IMAGE", "FRONTEND_IMAGE")
    ):
        raise ProvisionError("REGISTRY_SERVER does not match either selected image's registry.")
    for key in ("REGISTRY_USERNAME", "REGISTRY_PASSWORD"):
        require_setting(commands, values, key)


def check_images(values: dict[str, str]) -> None:
    """Reject demo images and require consent to preserve mutable defaults."""
    mutable = []
    for key in ("BACKEND_IMAGE", "FRONTEND_IMAGE"):
        image = values[key]
        if not re.fullmatch(
            r"[A-Za-z0-9][A-Za-z0-9._:/-]*(?:@sha256:[a-fA-F0-9]{64})?", image
        ) or "://" in image:
            raise ProvisionError(f"{key} must be a container image reference, not a URL or placeholder.")
        repository = image.split("@", 1)[0].split(":")[0]
        if repository == "mcr.microsoft.com/azuredocs/containerapps-helloworld":
            raise ProvisionError(f"{key} is the Bicep demo image; supply the actual application image.")
        if "@" not in image and (":" not in image.rsplit("/", 1)[-1] or image.endswith(":latest")):
            mutable.append(key)
    if mutable:
        confirm(
            "Keep explicitly configured latest/untagged images for "
            + ", ".join(mutable)
            + "? A new revision may pull different code; a version tag or digest is safer."
        )


def show_target(commands: Commands, name: str, values: dict[str, str]) -> None:
    """Display only the non-secret target and image selections."""
    print(commands.redact(f"\nProvisioning target: {name}"))
    for key in ("AZURE_SUBSCRIPTION_ID", "AZURE_LOCATION", "BACKEND_IMAGE", "FRONTEND_IMAGE"):
        print(commands.redact(f"  {key}: {values[key]}"))
    for key in ("AZURE_RESOURCE_GROUP", "POSTGRES_ADMIN_LOGIN", "POSTGRES_DB", "CUSTOM_DOMAIN_NAME"):
        print(commands.redact(f"  {key}: {values.get(key) or '(Bicep default)'}"))
    print(
        "This applies Bicep and its provider/domain hooks using these images; "
        "it does not build, publish, or release anything.\n"
        "Azure will validate deployment permissions and image access. "
        "Existing database credentials will be re-applied."
    )


def persist_settings(
    commands: Commands, name: str, saved: dict[str, str], values: dict[str, str],
    *, existing: bool,
) -> None:
    """Save confirmed additions only and verify their exact stored values."""
    if existing:
        if commands.settings(name) != saved:
            raise ProvisionError("The azd environment changed during setup. Rerun without overwriting it.")
    else:
        if (commands.root / ".azure" / name).exists():
            raise ProvisionError("Local environment configuration appeared during setup. Rerun without replacing it.")
        commands.capture(
            "azd", "env", "new", name, "--subscription", values["AZURE_SUBSCRIPTION_ID"],
            "--location", values["AZURE_LOCATION"], "--no-prompt",
        )
        saved = commands.settings(name)
        if any(saved.get(key) and saved[key] != value for key, value in values.items()):
            raise ProvisionError("azd initialized conflicting settings. Review the environment before retrying.")
    additions = {key: value for key, value in values.items() if saved.get(key) != value}
    if additions:
        print("Saving missing local settings: " + ", ".join(sorted(additions)))
        commands.save_missing(name, additions)
    stored = commands.settings(name)
    expected = saved | values
    if any(stored.get(key) != value for key, value in expected.items()):
        raise ProvisionError(
            "azd did not preserve configuration exactly. Restore the original values "
            "before retrying; provisioning was not started."
        )
    commands.protect(stored)


def run(commands: Commands, environment: str) -> None:
    """Prepare an explicitly confirmed deployment, then delegate to azd."""
    account = check_tools(commands)
    name, existing = select_environment(commands, environment)
    saved = commands.settings(name) if existing else {}
    values = configured_values(commands, name, saved)
    complete_settings(commands, values, saved, account, existing=existing)
    show_target(commands, name, values)
    additions = any(saved.get(key) != value for key, value in values.items())
    if additions:
        print(
            "Missing settings will be saved to .azure/<environment>/.env: "
            "gitignored plaintext, NOT encrypted storage."
        )
    confirm("Apply infrastructure to this target now?")
    persist_settings(commands, name, saved, values, existing=existing)
    commands.provision(name)
    print(
        "azd provisioning completed. Review any deferred custom-domain warnings "
        "and verify the website and API health before treating the rollout as healthy."
    )


def main() -> int:
    """Handle CLI arguments and report failures without credential traces."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("environment", nargs="?", default="", help="existing or explicitly named azd environment")
    args = parser.parse_args()
    commands = Commands(Path(__file__).resolve().parents[1])
    try:
        if sys.version_info < (3, 11):
            raise ProvisionError("Provisioning requires Python 3.11 or newer.")
        run(commands, args.environment)
    except (KeyboardInterrupt, EOFError):
        print(
            "\nCancelled. If azd had already started, verify Azure's deployment state; "
            "cancellation does not roll back changes.",
            file=sys.stderr,
        )
        return 130
    except (ProvisionError, OSError) as error:
        print(f"error: {commands.redact(str(error))}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
