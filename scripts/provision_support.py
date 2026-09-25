"""Secret-safe local configuration and command execution for provisioning."""

import getpass
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from urllib.parse import quote, quote_plus
import warnings


class ProvisionError(Exception):
    """An actionable provisioning error that is safe to display."""


def prompt(label: str, *, default: str = "", secret: bool = False) -> str:
    """Read a setting without falling back to echoed secret input."""
    if not sys.stdin.isatty():
        raise ProvisionError(
            f"{label.split(' (')[0]} requires interactive input. "
            "Restore the existing azd configuration, supply missing settings through "
            "environment variables, or rerun in a terminal."
        )
    suffix = f" [{default}]" if default else ""
    if not secret:
        return input(f"{label}{suffix}: ").strip() or default
    with warnings.catch_warnings():
        warnings.simplefilter("error", getpass.GetPassWarning)
        try:
            return getpass.getpass(f"{label}: ")
        except getpass.GetPassWarning:
            raise ProvisionError(
                "Cannot read a secret without echo. Restore it in the azd environment "
                "or rerun from a terminal with secure input."
            ) from None


def confirm(message: str) -> None:
    """Require explicit consent, with cancellation as the default."""
    if prompt(f"{message} [y/N]").lower() not in {"y", "yes"}:
        raise ProvisionError("Cancelled; provisioning was not started.")


def validate_values(values: dict[str, str]) -> None:
    """Reject values that supported azd dotenv writers cannot preserve."""
    for key, value in values.items():
        if "\0" in value:
            raise ProvisionError(f"{key} contains a NUL character and cannot be an environment value.")
        if value.endswith(("\\", '"')):
            raise ProvisionError(
                f"{key} ends with a backslash or double quote, which azd's dotenv "
                "writer cannot reliably round-trip. Nothing will be saved or provisioned. "
                "Keep the original credential and resolve azd storage compatibility "
                "before retrying; do not replace a production password to work around this."
            )


def dotenv(values: dict[str, str]) -> str:
    """Encode literal values for azd's godotenv parser, not for a shell."""
    validate_values(values)
    escapes = {
        "\\": "\\\\",
        "\n": "\\n",
        "\r": "\\r",
        '"': '\\"',
        "!": "\\!",
        "$": "\\$",
        "`": "\\`",
    }
    lines = []
    for key, value in sorted(values.items()):
        if not re.fullmatch(r"[A-Z][A-Z0-9_]*", key):
            raise ProvisionError("Cannot store an invalid environment key.")
        escaped = "".join(escapes.get(char, char) for char in value)
        lines.append(f'{key}="{escaped}"')
    return "\n".join(lines) + "\n"


class Commands:
    """Run CLI commands without shell evaluation or disclosing known secrets."""

    def __init__(self, root: Path, environment: dict[str, str] | None = None):
        self.root = root
        self.environment = dict(os.environ if environment is None else environment)
        self.secrets: set[str] = set()
        self.protect(self.environment)

    def protect(self, values: dict[str, str]) -> None:
        """Register secret values and common encodings for output redaction."""
        for key, value in values.items():
            if not value or not re.search(
                r"PASSWORD|TOKEN|SECRET|KEY|CONNECTION_STRING|DATABASE_URL", key
            ):
                continue
            self.secrets.update(
                {
                    value,
                    quote(value, safe=""),
                    quote_plus(value),
                    json.dumps(value)[1:-1],
                    json.dumps(value, ensure_ascii=False)[1:-1],
                }
            )
            self.secrets.update(part for part in value.splitlines() if part)

    def redact(self, text: str) -> str:
        """Remove secrets before displaying subprocess output."""
        for secret in sorted(self.secrets, key=len, reverse=True):
            text = text.replace(secret, "[redacted]")
        return text

    def capture(self, *args: str, private: bool = False) -> str:
        """Capture output; suppress untrusted diagnostics while loading secrets."""
        result = subprocess.run(
            args,
            cwd=self.root,
            env=self.environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            encoding="utf-8",
            errors="replace",
            check=False,
        )
        if result.returncode:
            detail = "" if private else self.redact(result.stderr.strip())
            message = f"{args[0]} {args[1]} failed (exit {result.returncode})."
            if detail:
                message += f"\n{detail}"
            raise ProvisionError(message)
        return result.stdout

    def json(self, *args: str, private: bool = False) -> object:
        """Read structured CLI output without including its contents in errors."""
        text = self.capture(*args, private=private)
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            raise ProvisionError(
                f"{args[0]} {args[1]} returned invalid JSON; no provisioning was started."
            ) from None

    def settings(self, name: str) -> dict[str, str]:
        """Read an explicitly selected azd environment into memory only."""
        try:
            data = self.json(
                "azd", "env", "get-values", "--environment", name,
                "--output", "json", "--no-prompt", private=True,
            )
        except ProvisionError as error:
            raise ProvisionError(
                f"{error}\nRestore the selected environment's original .env file. "
                "Raw diagnostics are suppressed because they can contain credentials."
            ) from None
        if not isinstance(data, dict) or not all(
            isinstance(key, str) and isinstance(value, str)
            for key, value in data.items()
        ):
            raise ProvisionError(
                "Cannot read azd settings. Restore the environment's .env file; "
                "its values have not been printed."
            )
        self.protect(data)
        return data

    def save_missing(self, name: str, values: dict[str, str]) -> None:
        """Import only missing settings through a private, short-lived file."""
        with tempfile.TemporaryDirectory(prefix="component-provision-") as directory:
            path = Path(directory) / "settings.env"
            descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as file:
                file.write(dotenv(values))
            self.capture(
                "azd", "env", "set", "--environment", name,
                "--file", str(path), "--no-prompt", private=True,
            )
        path = self.root / ".azure" / name / ".env"
        if not path.is_file():
            raise ProvisionError("azd did not save a local .env; provisioning was not started.")
        path.chmod(0o600)

    def provision(self, name: str) -> None:
        """Stream redacted azd output and propagate failure or interruption."""
        args = ["azd", "provision", "--environment", name, "--no-prompt"]
        with subprocess.Popen(
            args,
            cwd=self.root,
            env=self.environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            encoding="utf-8",
            errors="replace",
            bufsize=1,
        ) as process:
            try:
                assert process.stdout is not None
                for line in process.stdout:
                    print(self.redact(line), end="", flush=True)
                status = process.wait()
            except KeyboardInterrupt:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                raise
        if status:
            raise ProvisionError(
                f"azd provision failed (exit {status}); resources may be partially updated."
            )
