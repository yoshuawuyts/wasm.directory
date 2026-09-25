"""Offline az/azd command doubles; no authenticated CLI execution is allowed."""

import json
from pathlib import Path
import re
import sys

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))

from provision_support import Commands, ProvisionError


SUBSCRIPTION = "11111111-1111-1111-1111-111111111111"
PASSWORD = "test-only-NOT-a-real-password!$LITERAL"


def settings(name: str = "test-env") -> dict[str, str]:
    """Return synthetic configuration without production identifiers."""
    return {
        "AZURE_ENV_NAME": name,
        "AZURE_SUBSCRIPTION_ID": SUBSCRIPTION,
        "AZURE_LOCATION": "test-region",
        "AZURE_RESOURCE_GROUP": "rg-test-env",
        "POSTGRES_ADMIN_PASSWORD": PASSWORD,
        "BACKEND_IMAGE": "ghcr.io/example/test-backend:1.2.3",
        "FRONTEND_IMAGE": "ghcr.io/example/test-frontend:4.5.6",
        "CUSTOM_DOMAIN_NAME": "registry.example",
        "BACKEND_MAX_REPLICAS": "4",
        "LOG_ANALYTICS_DAILY_QUOTA_GB": "2",
    }


class FakeCommands(Commands):
    """Simulate local CLI state and reject every unrecognized command."""

    def __init__(
        self, root: Path, saved: dict[str, str] | None = None,
        environment: dict[str, str] | None = None,
    ):
        super().__init__(root, environment={} if environment is None else environment)
        (root / "infra").mkdir()
        (root / "infra" / "main.bicepparam").write_text(
            (SCRIPTS.parent / "infra" / "main.bicepparam").read_text(encoding="utf-8"),
            encoding="utf-8",
        )
        self.calls: list[tuple[str, ...]] = []
        self.environments = {} if saved is None else {saved["AZURE_ENV_NAME"]: dict(saved)}
        self.default = next(iter(self.environments), "")
        self.account = {"id": SUBSCRIPTION, "state": "Enabled"}
        self.failure: tuple[str, ...] | None = None
        self.bad_roundtrip = False
        self.imports: list[tuple[Path, int, str]] = []
        self.provisions: list[str] = []

    def capture(self, *args: str, private: bool = False) -> str:
        self.calls.append(args)
        if self.failure and args[:len(self.failure)] == self.failure:
            raise ProvisionError("Simulated CLI failure.")
        if args == ("azd", "env", "set", "--help"):
            return "azd env set --file"
        if args == ("az", "account", "show", "--output", "json"):
            return json.dumps(self.account)
        if args == ("az", "account", "get-access-token", "--query", "expiresOn", "--output", "tsv"):
            return "synthetic expiry, not an access token"
        if args == ("azd", "auth", "login", "--check-status"):
            return ""
        if args[:3] == ("azd", "env", "list"):
            return json.dumps([
                {"Name": name, "IsDefault": name == self.default, "HasLocal": True}
                for name in self.environments
            ])
        if args[:3] == ("azd", "env", "get-values"):
            return json.dumps(self.environments[args[args.index("--environment") + 1]])
        if args[:3] == ("azd", "env", "new"):
            name = args[3]
            if name in self.environments:
                raise ProvisionError("Environment already exists.")
            self.environments[name] = {
                "AZURE_ENV_NAME": name,
                "AZURE_SUBSCRIPTION_ID": args[args.index("--subscription") + 1],
                "AZURE_LOCATION": args[args.index("--location") + 1],
            }
            self.default = name
            return ""
        if args[:3] == ("azd", "env", "set"):
            return self.import_settings(args)
        raise AssertionError(f"Unexpected CLI command: {args!r}")

    def import_settings(self, args: tuple[str, ...]) -> str:
        """Model literal dotenv imports; serializer byte expectations are tested separately."""
        name = args[args.index("--environment") + 1]
        path = Path(args[args.index("--file") + 1])
        text = path.read_text(encoding="utf-8")
        self.imports.append((path, path.stat().st_mode & 0o777, text))
        escapes = {"n": "\n", "r": "\r"}
        for line in text.splitlines():
            key, quoted = line.split("=", 1)
            assert quoted.startswith('"') and quoted.endswith('"')
            value = re.sub(
                r"\\(.)", lambda match: escapes.get(match[1], match[1]), quoted[1:-1]
            )
            self.environments[name][key] = "changed" if self.bad_roundtrip else value
        env_file = self.root / ".azure" / name / ".env"
        env_file.parent.mkdir(parents=True, exist_ok=True)
        env_file.write_text(text, encoding="utf-8")
        return ""

    def provision(self, name: str) -> None:
        self.calls.append(("azd", "provision", "--environment", name, "--no-prompt"))
        self.provisions.append(name)
        if self.failure == ("azd", "provision"):
            raise ProvisionError("Simulated provisioning failure.")
