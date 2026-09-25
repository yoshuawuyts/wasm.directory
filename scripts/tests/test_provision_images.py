"""Offline image selection and registry authentication coverage."""

import contextlib
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from provision_fakes import FakeCommands, settings
import provision
from provision_support import ProvisionError


class ImageTests(unittest.TestCase):
    """Preserve deliberate images without introducing default deployments."""

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

    def invoke(self, commands, answers=("yes",)):
        with patch("builtins.input", side_effect=answers), contextlib.redirect_stdout(self.output):
            provision.run(commands, "")

    def test_public_ghcr_images_do_not_require_registry_credentials(self):
        commands = FakeCommands(self.root, settings())
        with patch("getpass.getpass", side_effect=AssertionError("Unexpected credential prompt")):
            self.invoke(commands)
        self.assertFalse(commands.imports)
        self.assertNotIn("REGISTRY_PASSWORD", commands.environments["test-env"])

    def test_saved_private_registry_credentials_are_unchanged(self):
        saved = settings() | {
            "REGISTRY_SERVER": "ghcr.io",
            "REGISTRY_USERNAME": "example",
            "REGISTRY_PASSWORD": "test-only-pull-token",
        }
        commands = FakeCommands(self.root, saved)
        self.invoke(commands)
        self.assertEqual(commands.environments["test-env"], saved)
        self.assertFalse(commands.imports)
        self.assertNotIn(saved["REGISTRY_PASSWORD"], self.output.getvalue())
        self.assertNotIn(saved["REGISTRY_PASSWORD"], repr(commands.calls))

    def test_incomplete_private_registry_requires_original_credential(self):
        saved = settings() | {"REGISTRY_SERVER": "ghcr.io", "REGISTRY_USERNAME": "example"}
        commands = FakeCommands(self.root, saved)
        with patch("sys.stdin.isatty", return_value=False):
            with self.assertRaisesRegex(ProvisionError, "REGISTRY_PASSWORD"):
                self.invoke(commands)
        self.assertFalse(commands.provisions)

    def test_registry_password_is_entered_without_echo_and_saved_locally(self):
        saved = settings() | {"REGISTRY_SERVER": "ghcr.io", "REGISTRY_USERNAME": "example"}
        commands = FakeCommands(self.root, saved)
        token = "test-only-pull-token"
        with patch("getpass.getpass", return_value=token):
            self.invoke(commands)
        self.assertEqual(commands.environments["test-env"]["REGISTRY_PASSWORD"], token)
        self.assertNotIn(token, self.output.getvalue())
        self.assertNotIn(token, repr(commands.calls))

    def test_unmatched_registry_does_not_discard_credentials(self):
        saved = settings() | {
            "REGISTRY_SERVER": "different.example",
            "REGISTRY_USERNAME": "example",
            "REGISTRY_PASSWORD": "test-only-pull-token",
        }
        commands = FakeCommands(self.root, saved)
        with self.assertRaisesRegex(ProvisionError, "does not match"):
            self.invoke(commands)
        self.assertEqual(commands.environments["test-env"], saved)
        self.assertFalse(commands.provisions)

    def test_orphan_registry_credentials_require_server_recovery(self):
        saved = settings() | {
            "REGISTRY_USERNAME": "example", "REGISTRY_PASSWORD": "test-only-pull-token",
        }
        commands = FakeCommands(self.root, saved)
        with patch("sys.stdin.isatty", return_value=False):
            with self.assertRaisesRegex(ProvisionError, "REGISTRY_SERVER"):
                self.invoke(commands)
        self.assertFalse(commands.provisions)

    def test_ci_pull_token_is_not_automatically_used(self):
        commands = FakeCommands(self.root, settings(), {"GHCR_PULL_TOKEN": "test-only-ci-token"})
        self.invoke(commands)
        self.assertNotIn("REGISTRY_PASSWORD", commands.environments["test-env"])
        self.assertNotIn("test-only-ci-token", self.output.getvalue())

    def test_missing_image_never_falls_back_to_bicep_placeholder(self):
        saved = settings()
        del saved["BACKEND_IMAGE"]
        commands = FakeCommands(self.root, saved)
        with patch("sys.stdin.isatty", return_value=False):
            with self.assertRaisesRegex(ProvisionError, "BACKEND_IMAGE"):
                self.invoke(commands)
        self.assertFalse(commands.provisions)

    def test_placeholder_images_are_rejected_even_with_a_digest(self):
        for suffix in (":latest", "@sha256:" + "a" * 64):
            with self.subTest(suffix=suffix), tempfile.TemporaryDirectory() as directory:
                saved = settings()
                saved["BACKEND_IMAGE"] = "mcr.microsoft.com/azuredocs/containerapps-helloworld" + suffix
                commands = FakeCommands(Path(directory), saved)
                with self.assertRaisesRegex(ProvisionError, "demo image"):
                    self.invoke(commands)
                self.assertFalse(commands.provisions)

    def test_latest_and_untagged_images_require_additional_confirmation(self):
        for image in ("ghcr.io/example/backend:latest", "localhost:5000/example/backend"):
            with self.subTest(image=image), tempfile.TemporaryDirectory() as directory:
                saved = settings() | {"BACKEND_IMAGE": image}
                commands = FakeCommands(Path(directory), saved)
                self.invoke(commands, ("yes", "yes"))
                self.assertEqual(commands.environments["test-env"]["BACKEND_IMAGE"], image)
                self.assertEqual(commands.provisions, ["test-env"])

    def test_rejecting_mutable_image_never_changes_it_or_provisions(self):
        saved = settings() | {"BACKEND_IMAGE": "ghcr.io/example/backend:latest"}
        commands = FakeCommands(self.root, saved)
        with self.assertRaisesRegex(ProvisionError, "Cancelled"):
            self.invoke(commands, ("no",))
        self.assertEqual(commands.environments["test-env"], saved)
        self.assertFalse(commands.provisions)

    def test_digest_pinned_image_needs_only_apply_confirmation(self):
        saved = settings() | {"BACKEND_IMAGE": "ghcr.io/example/backend:latest@sha256:" + "a" * 64}
        commands = FakeCommands(self.root, saved)
        self.invoke(commands)
        self.assertEqual(commands.provisions, ["test-env"])

    def test_image_urls_with_credentials_are_rejected_without_echo(self):
        values = settings() | {"BACKEND_IMAGE": "https://user:test-only-secret@registry.example/image"}
        with self.assertRaises(ProvisionError) as caught:
            provision.check_images(values)
        self.assertNotIn("test-only-secret", str(caught.exception))

    def test_empty_optional_database_values_do_not_override_defaults(self):
        commands = FakeCommands(self.root, settings() | {"POSTGRES_DB": ""})
        with self.assertRaisesRegex(ProvisionError, "POSTGRES_DB is explicitly empty"):
            self.invoke(commands)
        self.assertFalse(commands.provisions)


if __name__ == "__main__":
    unittest.main()
