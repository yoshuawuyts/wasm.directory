"""Tests for secret handling and subprocess boundaries, using no live CLI."""

import contextlib
import getpass
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import MagicMock, patch
from urllib.parse import quote
import warnings

from provision_fakes import PASSWORD
import provision
from provision_support import Commands, ProvisionError, dotenv, prompt


class SupportTests(unittest.TestCase):
    """Verify errors, terminal input, and subprocess output do not disclose secrets."""

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.commands = Commands(self.root, {})
        self.commands.protect({"POSTGRES_ADMIN_PASSWORD": PASSWORD})
        self.addCleanup(patch.stopall)
        patch("subprocess.run", side_effect=AssertionError("Live CLI execution forbidden")).start()
        patch("subprocess.Popen", side_effect=AssertionError("Live CLI execution forbidden")).start()

    def test_dotenv_escapes_literals_without_interpolating_them(self):
        self.assertEqual(
            dotenv({"POSTGRES_ADMIN_PASSWORD": " ' \" $VALUE ${VALUE} `cmd` \\!\t\n\r"}),
            'POSTGRES_ADMIN_PASSWORD=" \' \\" \\$VALUE \\${VALUE} \\`cmd\\` \\\\\\!\t\\n\\r"\n',
        )

    def test_invalid_keys_and_nul_are_rejected_without_values(self):
        for data in ({"BAD\nKEY": PASSWORD}, {"PASSWORD": PASSWORD + "\0"}):
            with self.subTest(data=list(data)):
                with self.assertRaises(ProvisionError) as caught:
                    dotenv(data)
                self.assertNotIn(PASSWORD, str(caught.exception))

    def test_unsupported_azd_endings_are_rejected_before_any_import(self):
        for value in (PASSWORD + "\\", PASSWORD + '"'):
            with self.subTest(ending=value[-1]):
                with self.assertRaisesRegex(ProvisionError, "round-trip"):
                    dotenv({"POSTGRES_ADMIN_PASSWORD": value})

    def test_capture_uses_no_shell_and_no_secret_arguments(self):
        result = subprocess.CompletedProcess([], 0, stdout="{}", stderr="")
        with patch("subprocess.run", return_value=result) as run:
            self.commands.capture("azd", "env", "list")
        args, kwargs = run.call_args
        self.assertEqual(args[0], ("azd", "env", "list"))
        self.assertNotIn("shell", kwargs)
        self.assertEqual(kwargs["stdin"], subprocess.DEVNULL)
        self.assertNotIn(PASSWORD, repr(args[0]))

    def test_command_failure_redacts_stderr(self):
        result = subprocess.CompletedProcess([], 42, stdout="", stderr="failure: " + PASSWORD)
        with patch("subprocess.run", return_value=result):
            with self.assertRaises(ProvisionError) as caught:
                self.commands.capture("azd", "auth")
        self.assertIn("exit 42", str(caught.exception))
        self.assertIn("[redacted]", str(caught.exception))
        self.assertNotIn(PASSWORD, str(caught.exception))

    def test_private_load_failure_hides_even_unknown_secret_diagnostics(self):
        unknown = "test-only-unknown-secret-in-malformed-file"
        result = subprocess.CompletedProcess([], 1, stdout=unknown, stderr=unknown)
        with patch("subprocess.run", return_value=result):
            with self.assertRaises(ProvisionError) as caught:
                self.commands.settings("test-env")
        self.assertNotIn(unknown, str(caught.exception))

    def test_redaction_handles_plain_json_url_and_multiline_variants(self):
        secret = 'test-only-quoted " value\nsecond-line'
        self.commands.protect({"REGISTRY_PASSWORD": secret})
        for variant in (secret, json.dumps(secret)[1:-1], quote(secret, safe=""), "second-line"):
            with self.subTest(variant=variant):
                self.assertNotIn(variant, self.commands.redact("value: " + variant))

    def test_secret_prompt_never_uses_echoed_input(self):
        with patch("sys.stdin.isatty", return_value=True):
            with patch("builtins.input", side_effect=AssertionError("Echoed secret input")):
                with patch("getpass.getpass", return_value="  " + PASSWORD + "  "):
                    self.assertEqual(
                        prompt("POSTGRES_ADMIN_PASSWORD", secret=True), "  " + PASSWORD + "  "
                    )

    def test_getpass_fallback_warning_is_an_error_before_echo(self):
        def unavailable(_label):
            warnings.warn("Cannot control echo", getpass.GetPassWarning)
            self.fail("getpass must not continue into its echoed fallback")

        with patch("sys.stdin.isatty", return_value=True):
            with patch("getpass.getpass", side_effect=unavailable):
                with self.assertRaisesRegex(ProvisionError, "without echo"):
                    prompt("POSTGRES_ADMIN_PASSWORD", secret=True)

    def test_secret_prompt_without_terminal_does_not_call_getpass(self):
        with patch("sys.stdin.isatty", return_value=False):
            with patch("getpass.getpass") as getpass_call:
                with self.assertRaisesRegex(ProvisionError, "interactive"):
                    prompt("POSTGRES_ADMIN_PASSWORD", secret=True)
        getpass_call.assert_not_called()

    def test_temp_import_permissions_cleanup_and_argument_safety(self):
        seen = []

        def capture(*args, private=False):
            path = Path(args[args.index("--file") + 1])
            seen.append(path)
            self.assertTrue(private)
            if os.name != "nt":
                self.assertEqual(path.stat().st_mode & 0o777, 0o600)
                self.assertEqual(path.parent.stat().st_mode & 0o777, 0o700)
            self.assertIn("POSTGRES_ADMIN_PASSWORD=", path.read_text(encoding="utf-8"))
            self.assertNotIn(PASSWORD, repr(args))
            env_file = self.root / ".azure" / "test-env" / ".env"
            env_file.parent.mkdir(parents=True)
            env_file.write_text("synthetic local store", encoding="utf-8")
            return ""

        with patch.object(self.commands, "capture", side_effect=capture):
            self.commands.save_missing("test-env", {"POSTGRES_ADMIN_PASSWORD": PASSWORD})
        self.assertFalse(seen[0].exists())
        self.assertFalse(seen[0].parent.exists())
        if os.name != "nt":
            self.assertEqual((self.root / ".azure" / "test-env" / ".env").stat().st_mode & 0o777, 0o600)

    def test_keyboard_interrupt_cleans_secret_import(self):
        seen = []

        def interrupted(*args, private=False):
            seen.append(Path(args[args.index("--file") + 1]))
            raise KeyboardInterrupt

        with patch.object(self.commands, "capture", side_effect=interrupted):
            with self.assertRaises(KeyboardInterrupt):
                self.commands.save_missing("test-env", {"POSTGRES_ADMIN_PASSWORD": PASSWORD})
        self.assertFalse(seen[0].exists())

    def test_provision_stream_redacts_secrets_and_uses_selected_environment(self):
        process = MagicMock()
        process.__enter__.return_value = process
        process.stdout = iter(["status\n", "value=" + PASSWORD + "\n"])
        process.wait.return_value = 0
        output = io.StringIO()
        with patch("subprocess.Popen", return_value=process) as popen:
            with contextlib.redirect_stdout(output):
                self.commands.provision("chosen-env")
        self.assertEqual(
            popen.call_args.args[0],
            ["azd", "provision", "--environment", "chosen-env", "--no-prompt"],
        )
        self.assertNotIn(PASSWORD, output.getvalue())
        self.assertIn("[redacted]", output.getvalue())

    def test_provision_failure_warns_about_partial_updates(self):
        process = MagicMock()
        process.__enter__.return_value = process
        process.stdout = iter([])
        process.wait.return_value = 7
        with patch("subprocess.Popen", return_value=process):
            with self.assertRaisesRegex(ProvisionError, "partially updated"):
                self.commands.provision("chosen-env")

    def test_interrupt_terminates_only_the_started_child(self):
        process = MagicMock()
        process.__enter__.return_value = process
        process.stdout.__iter__.side_effect = KeyboardInterrupt
        with patch("subprocess.Popen", return_value=process):
            with self.assertRaises(KeyboardInterrupt):
                self.commands.provision("chosen-env")
        process.terminate.assert_called_once()
        process.wait.assert_called_once_with(timeout=5)

    def test_main_handles_eof_without_claiming_rollback(self):
        error_output = io.StringIO()
        with patch("sys.argv", ["provision.py"]):
            with patch("provision.run", side_effect=EOFError):
                with contextlib.redirect_stderr(error_output):
                    self.assertEqual(provision.main(), 130)
        self.assertIn("does not roll back", error_output.getvalue())


if __name__ == "__main__":
    unittest.main()
