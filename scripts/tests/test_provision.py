"""Offline configuration, confirmation, and failure-path coverage."""

import contextlib
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from provision_fakes import FakeCommands, PASSWORD, SUBSCRIPTION, settings
import provision
from provision_support import ProvisionError


class ProvisionTests(unittest.TestCase):
    """Exercise real helper control flow with strict CLI stubs."""

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.output = io.StringIO()
        self.addCleanup(patch.stopall)
        patch("subprocess.run", side_effect=AssertionError("Live CLI execution forbidden")).start()
        patch("subprocess.Popen", side_effect=AssertionError("Live CLI execution forbidden")).start()
        patch("provision.shutil.which", return_value="/stub/tool").start()
        patch("sys.stdin.isatty", return_value=True).start()

    def invoke(self, commands, answers=("yes",), environment=""):
        with patch("builtins.input", side_effect=answers), contextlib.redirect_stdout(self.output):
            provision.run(commands, environment)

    def test_existing_settings_are_unchanged_and_no_setup_is_called(self):
        saved = settings()
        commands = FakeCommands(self.root, saved)
        self.invoke(commands)
        self.assertEqual(commands.environments["test-env"], saved)
        self.assertEqual(commands.provisions, ["test-env"])
        self.assertFalse(commands.imports)
        self.assertFalse(any(call[:3] == ("azd", "env", "new") for call in commands.calls))
        self.assertNotIn(PASSWORD, self.output.getvalue())
        self.assertIn("BACKEND_IMAGE", self.output.getvalue())
        self.assertIn("--no-prompt", commands.calls[-1])
        self.assertTrue(all(call[0] in {"az", "azd"} for call in commands.calls))
        self.assertFalse(any("release" in call or "up" in call for call in commands.calls))

    def test_explicit_environment_does_not_change_default(self):
        commands = FakeCommands(self.root, settings())
        commands.environments["second"] = settings("second")
        self.invoke(commands, environment="second")
        self.assertEqual(commands.provisions, ["second"])
        self.assertEqual(commands.default, "test-env")
        get_calls = [call for call in commands.calls if call[:3] == ("azd", "env", "get-values")]
        self.assertTrue(all(call[call.index("--environment") + 1] == "second" for call in get_calls))

    def test_environment_input_selects_target(self):
        commands = FakeCommands(self.root, settings(), {"AZURE_ENV_NAME": "test-env"})
        self.invoke(commands)
        self.assertEqual(commands.provisions, ["test-env"])

    def test_ambiguous_environment_requires_selection(self):
        commands = FakeCommands(self.root, settings())
        commands.environments["second"] = settings("second")
        commands.default = ""
        self.invoke(commands, ("second", "yes"))
        self.assertEqual(commands.provisions, ["second"])

    def test_ambiguous_environment_without_terminal_fails(self):
        commands = FakeCommands(self.root, settings())
        commands.default = ""
        with patch("sys.stdin.isatty", return_value=False):
            with self.assertRaisesRegex(ProvisionError, "interactive"):
                self.invoke(commands)
        self.assertFalse(commands.provisions)

    def test_invalid_environment_name_never_reaches_setup(self):
        commands = FakeCommands(self.root)
        with self.assertRaisesRegex(ProvisionError, "environment name"):
            self.invoke(commands, environment="../outside")
        self.assertFalse(commands.provisions)
        self.assertFalse(any(call[:3] == ("azd", "env", "new") for call in commands.calls))

    def test_environment_list_failure_is_not_treated_as_missing(self):
        commands = FakeCommands(self.root)
        commands.failure = ("azd", "env", "list")
        with self.assertRaises(ProvisionError):
            self.invoke(commands, environment="new")
        self.assertFalse(any(call[:3] == ("azd", "env", "new") for call in commands.calls))

    def test_existing_environment_case_is_not_changed(self):
        commands = FakeCommands(self.root, settings())
        with self.assertRaisesRegex(ProvisionError, "exact name"):
            self.invoke(commands, environment="TEST-ENV")
        self.assertFalse(any(call[:3] == ("azd", "env", "new") for call in commands.calls))

    def test_unlisted_local_configuration_is_not_reset(self):
        commands = FakeCommands(self.root)
        env_dir = self.root / ".azure" / "partial-env"
        env_dir.mkdir(parents=True)
        sentinel = env_dir / "config.json"
        sentinel.write_text("preserve this partial configuration", encoding="utf-8")
        with self.assertRaisesRegex(ProvisionError, "will not reinitialize"):
            self.invoke(commands, environment="partial-env")
        self.assertEqual(sentinel.read_text(encoding="utf-8"), "preserve this partial configuration")
        self.assertFalse(any(call[:3] == ("azd", "env", "new") for call in commands.calls))

    def test_malformed_environment_list_is_rejected(self):
        commands = FakeCommands(self.root)
        with patch.object(commands, "json", return_value={"not": "a list"}):
            with self.assertRaisesRegex(ProvisionError, "invalid environment list"):
                provision.select_environment(commands, "")

    def test_remote_only_environment_is_not_mutated(self):
        commands = FakeCommands(self.root)
        with patch.object(commands, "json", return_value=[
            {"Name": "remote", "IsDefault": True, "HasLocal": False}
        ]):
            with self.assertRaisesRegex(ProvisionError, "remote-only"):
                provision.select_environment(commands, "")
        self.assertFalse(commands.provisions)

    def test_missing_tool_fails_before_commands(self):
        commands = FakeCommands(self.root, settings())
        with patch("provision.shutil.which", return_value=None):
            with self.assertRaisesRegex(ProvisionError, "not installed"):
                self.invoke(commands)
        self.assertFalse(commands.calls)

    def test_unsupported_azd_fails_with_upgrade_guidance(self):
        commands = FakeCommands(self.root, settings())
        with patch.object(commands, "capture", return_value="no file flag"):
            with self.assertRaisesRegex(ProvisionError, "Update azd"):
                self.invoke(commands)
        self.assertFalse(commands.provisions)

    def test_az_and_azd_auth_failures_do_not_start_login_or_provision(self):
        for command, login in (
            (("az", "account", "show"), "az login"),
            (("az", "account", "get-access-token"), "az login"),
            (("azd", "auth", "login"), "azd auth login"),
        ):
            with self.subTest(command=command), tempfile.TemporaryDirectory() as directory:
                commands = FakeCommands(Path(directory), settings())
                commands.failure = command
                with self.assertRaisesRegex(ProvisionError, login):
                    self.invoke(commands)
                self.assertFalse(commands.provisions)
                self.assertNotIn(("az", "login"), commands.calls)
                self.assertNotIn(("azd", "auth", "login"), commands.calls)

    def test_disabled_subscription_is_rejected(self):
        commands = FakeCommands(self.root, settings())
        commands.account["state"] = "Disabled"
        with self.assertRaisesRegex(ProvisionError, "enabled subscription"):
            self.invoke(commands)
        self.assertFalse(commands.provisions)

    def test_subscription_mismatch_never_switches_account(self):
        saved = settings()
        saved["AZURE_SUBSCRIPTION_ID"] = "22222222-2222-2222-2222-222222222222"
        commands = FakeCommands(self.root, saved)
        with self.assertRaisesRegex(ProvisionError, "account set"):
            self.invoke(commands)
        self.assertFalse(any(call[:3] == ("az", "account", "set") for call in commands.calls))
        self.assertFalse(commands.provisions)

    def test_conflicting_inputs_are_reported_by_name_not_value(self):
        commands = FakeCommands(self.root, settings(), {
            "POSTGRES_ADMIN_PASSWORD": "test-only-different-password",
            "BACKEND_MAX_REPLICAS": "9",
        })
        with self.assertRaises(ProvisionError) as caught:
            self.invoke(commands)
        self.assertIn("POSTGRES_ADMIN_PASSWORD", str(caught.exception))
        self.assertIn("BACKEND_MAX_REPLICAS", str(caught.exception))
        self.assertNotIn("test-only-different-password", str(caught.exception))
        self.assertEqual(commands.environments["test-env"]["POSTGRES_ADMIN_PASSWORD"], PASSWORD)
        self.assertFalse(commands.provisions)

    def test_explicit_environment_conflict_with_process_is_rejected(self):
        commands = FakeCommands(self.root, settings(), {"AZURE_ENV_NAME": "another"})
        with self.assertRaisesRegex(ProvisionError, "AZURE_ENV_NAME"):
            self.invoke(commands, environment="test-env")
        self.assertFalse(commands.provisions)

    def test_missing_secret_requires_secure_recovery_noninteractively(self):
        saved = settings()
        del saved["POSTGRES_ADMIN_PASSWORD"]
        commands = FakeCommands(self.root, saved)
        with patch("sys.stdin.isatty", return_value=False):
            with self.assertRaisesRegex(ProvisionError, "POSTGRES_ADMIN_PASSWORD"):
                self.invoke(commands)
        self.assertFalse(commands.imports)
        self.assertFalse(commands.provisions)

    def test_missing_secret_is_prompted_and_preserved_exactly(self):
        saved = settings()
        del saved["POSTGRES_ADMIN_PASSWORD"]
        secret = " test-only 'quoted' \"double\" $VARIABLE ${BRACES} `echo` \\ backslash! \t "
        commands = FakeCommands(self.root, saved)
        with patch("getpass.getpass", return_value=secret) as getpass:
            self.invoke(commands)
        getpass.assert_called_once()
        self.assertEqual(commands.environments["test-env"]["POSTGRES_ADMIN_PASSWORD"], secret)
        self.assertNotIn(secret, self.output.getvalue())
        self.assertNotIn(secret, repr(commands.calls))
        self.assertFalse(commands.imports[0][0].exists())
        self.assertEqual(commands.provisions, ["test-env"])

    def test_missing_value_from_process_fills_only_gap(self):
        saved = settings()
        del saved["POSTGRES_ADMIN_PASSWORD"]
        commands = FakeCommands(self.root, saved, {"POSTGRES_ADMIN_PASSWORD": PASSWORD})
        self.invoke(commands)
        self.assertEqual(commands.environments["test-env"], settings())
        self.assertEqual(len(commands.imports), 1)
        self.assertNotIn("BACKEND_IMAGE=", commands.imports[0][2])

    def test_cancelling_apply_does_not_even_save_missing_inputs(self):
        saved = settings()
        del saved["POSTGRES_ADMIN_PASSWORD"]
        commands = FakeCommands(self.root, saved, {"POSTGRES_ADMIN_PASSWORD": PASSWORD})
        with self.assertRaisesRegex(ProvisionError, "Cancelled"):
            self.invoke(commands, ("no",))
        self.assertFalse(commands.imports)
        self.assertFalse(commands.provisions)
        self.assertEqual(commands.environments["test-env"], saved)

    def test_no_terminal_never_implicitly_confirms_apply(self):
        commands = FakeCommands(self.root, settings())
        with patch("sys.stdin.isatty", return_value=False):
            with self.assertRaisesRegex(ProvisionError, "interactive"):
                self.invoke(commands)
        self.assertFalse(commands.provisions)

    def test_changed_environment_stops_before_saving_or_provisioning(self):
        commands = FakeCommands(self.root, settings())
        with patch.object(commands, "settings", side_effect=[
            settings(), settings() | {"FRONTEND_MAX_REPLICAS": "5"},
        ]):
            with self.assertRaisesRegex(ProvisionError, "changed during setup"):
                self.invoke(commands)
        self.assertFalse(commands.imports)
        self.assertFalse(commands.provisions)

    def test_import_failure_cleans_temporary_file(self):
        saved = settings()
        del saved["POSTGRES_ADMIN_PASSWORD"]
        commands = FakeCommands(self.root, saved, {"POSTGRES_ADMIN_PASSWORD": PASSWORD})
        commands.failure = ("azd", "env", "set", "--environment")
        with self.assertRaises(ProvisionError):
            self.invoke(commands)
        call = next(call for call in commands.calls if "--file" in call)
        self.assertFalse(Path(call[call.index("--file") + 1]).exists())
        self.assertFalse(commands.provisions)

    def test_changed_imported_secret_stops_before_provisioning(self):
        saved = settings()
        del saved["POSTGRES_ADMIN_PASSWORD"]
        commands = FakeCommands(self.root, saved, {"POSTGRES_ADMIN_PASSWORD": PASSWORD})
        commands.bad_roundtrip = True
        with self.assertRaisesRegex(ProvisionError, "preserve configuration exactly"):
            self.invoke(commands)
        self.assertFalse(commands.provisions)

    def test_azd_storage_edge_cases_never_change_existing_credentials(self):
        for suffix in ("\\", '"'):
            with self.subTest(suffix=suffix), tempfile.TemporaryDirectory() as directory:
                saved = settings() | {"POSTGRES_ADMIN_PASSWORD": PASSWORD + suffix}
                commands = FakeCommands(Path(directory), saved)
                with self.assertRaisesRegex(ProvisionError, "round-trip"):
                    self.invoke(commands)
                self.assertEqual(commands.environments["test-env"], saved)
                self.assertFalse(commands.imports)
                self.assertFalse(commands.provisions)

    def test_provision_failure_is_not_reported_as_success(self):
        commands = FakeCommands(self.root, settings())
        commands.failure = ("azd", "provision")
        with self.assertRaises(ProvisionError):
            self.invoke(commands)
        self.assertNotIn("provisioning completed", self.output.getvalue())

    def test_cancelled_bootstrap_does_not_create_an_environment(self):
        commands = FakeCommands(self.root)
        with self.assertRaisesRegex(ProvisionError, "Cancelled"):
            self.invoke(commands, ("no",), "new-env")
        self.assertFalse(commands.environments)

    def test_guided_bootstrap_uses_existing_inputs_without_empty_defaults(self):
        inputs = settings("new-env")
        for key in ("AZURE_RESOURCE_GROUP", "CUSTOM_DOMAIN_NAME"):
            del inputs[key]
        commands = FakeCommands(self.root, environment=inputs)
        self.invoke(commands, ("yes", "", "", "", "", "yes", "yes"), "new-env")
        stored = commands.environments["new-env"]
        self.assertEqual(stored["POSTGRES_ADMIN_PASSWORD"], PASSWORD)
        self.assertEqual(stored["BACKEND_IMAGE"], inputs["BACKEND_IMAGE"])
        self.assertNotIn("POSTGRES_ADMIN_LOGIN", stored)
        self.assertNotIn("POSTGRES_DB", stored)
        self.assertNotIn("REGISTRY_PASSWORD", stored)
        self.assertEqual(commands.provisions, ["new-env"])
        self.assertIn("NOT encrypted", self.output.getvalue())
        new_call = next(call for call in commands.calls if call[:3] == ("azd", "env", "new"))
        self.assertEqual(new_call[new_call.index("--subscription") + 1], SUBSCRIPTION)
        self.assertNotIn(PASSWORD, repr(commands.calls))

    def test_environment_creation_failure_stops_before_import_or_provision(self):
        commands = FakeCommands(self.root, environment=settings("new-env"))
        commands.failure = ("azd", "env", "new")
        with self.assertRaises(ProvisionError):
            self.invoke(commands, ("yes", "", "", "yes", "yes"), "new-env")
        self.assertFalse(commands.imports)
        self.assertFalse(commands.provisions)

    def test_malformed_secret_settings_are_never_printed(self):
        commands = FakeCommands(self.root, settings())
        with patch.object(commands, "json", return_value={"POSTGRES_ADMIN_PASSWORD": [PASSWORD]}):
            with self.assertRaises(ProvisionError) as caught:
                commands.settings("test-env")
        self.assertNotIn(PASSWORD, str(caught.exception))

    def test_json_error_never_embeds_raw_payload(self):
        commands = FakeCommands(self.root)
        with patch.object(commands, "capture", return_value=PASSWORD):
            with self.assertRaises(ProvisionError) as caught:
                commands.json("azd", "env")
        self.assertNotIn(PASSWORD, str(caught.exception))


if __name__ == "__main__":
    unittest.main()
