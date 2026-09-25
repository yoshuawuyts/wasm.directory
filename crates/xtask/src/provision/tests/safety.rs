use std::sync::atomic::AtomicBool;

use super::{PASSWORD, values};
use crate::provision::{process, redactor::Redactor, store};

#[test]
fn dotenv_encodes_literal_values_without_evaluation() {
    let input = " ' \" $VALUE ${VALUE} `cmd` \\!\t\n\r";
    let encoded = store::dotenv(&values(&[("POSTGRES_ADMIN_PASSWORD", input)]))
        .expect("supported literal encodes");
    assert_eq!(
        encoded,
        "POSTGRES_ADMIN_PASSWORD=\" ' \\\" \\$VALUE \\${VALUE} \\`cmd\\` \\\\\\!\t\\n\\r\"\n"
    );
}

#[test]
fn dotenv_rejects_invalid_keys_without_values() {
    let error = store::dotenv(&values(&[("BAD\nKEY", PASSWORD)]))
        .expect_err("invalid key rejected")
        .to_string();
    assert!(!error.contains(PASSWORD));
}

#[test]
fn unsupported_azd_endings_are_rejected_before_import() {
    for suffix in ['\\', '"'] {
        let input = format!("{PASSWORD}{suffix}");
        let error = store::dotenv(&values(&[("POSTGRES_ADMIN_PASSWORD", &input)]))
            .expect_err("unsupported ending rejected")
            .to_string();
        assert!(error.contains("round-trip"));
        assert!(!error.contains(PASSWORD));
    }
}

#[test]
fn redaction_covers_plain_json_url_and_multiline_values() {
    let mut redactor = Redactor::default();
    let secret = "test-only \" value\nsecond-line";
    redactor.protect(&values(&[("REGISTRY_PASSWORD", secret)]));
    for variant in [
        secret,
        "test-only \\\" value\\nsecond-line",
        "test-only%20%22%20value%0Asecond-line",
        "test-only+%22+value%0Asecond-line",
        "second-line",
    ] {
        assert!(
            !redactor
                .redact(&format!("value: {variant}"))
                .contains(variant)
        );
    }
}

#[test]
fn redaction_covers_unicode_json_escapes_and_surrogates() {
    let mut redactor = Redactor::default();
    redactor.protect(&values(&[(
        "POSTGRES_ADMIN_PASSWORD",
        "test-\u{e9}-\u{1f600}",
    )]));
    for encoded in ["test-\u{e9}-\u{1f600}", "test-\\u00e9-\\ud83d\\ude00"] {
        assert!(!redactor.redact(encoded).contains(encoded));
    }
}

#[test]
fn redactor_debug_does_not_disclose_secrets() {
    let mut redactor = Redactor::default();
    redactor.protect(&values(&[("POSTGRES_ADMIN_PASSWORD", PASSWORD)]));
    assert!(!format!("{redactor:?}").contains(PASSWORD));
}

#[test]
fn redaction_covers_html_escaped_json() {
    let mut redactor = Redactor::default();
    redactor.protect(&values(&[("POSTGRES_ADMIN_PASSWORD", "test-<&>")]));
    assert_eq!(redactor.redact("test-\\u003c\\u0026\\u003e"), "[redacted]");
}

#[test]
fn empty_command_is_rejected() {
    assert!(process::command(&[]).is_err());
}

#[test]
fn command_arguments_are_not_interpreted_by_a_shell() {
    let command = process::command(&["azd", "env", "get-values", "--environment", "name;$(echo)"])
        .expect("construct literal command");
    assert_eq!(command.get_program(), "azd");
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(
        args.last().expect("literal environment argument"),
        &"name;$(echo)"
    );
}

#[test]
fn cancellation_before_start_does_not_spawn_a_command() {
    let directory = tempfile::tempdir().expect("create empty fixture directory");
    let path = directory.path().join("missing-cli");
    let command = process::command(&[path.to_str().expect("UTF-8 fixture path")])
        .expect("construct sentinel command");
    let error = process::run(command, &AtomicBool::new(true), |_, _| {
        panic!("cancelled command must not emit output");
    })
    .expect_err("cancelled before process start")
    .to_string();
    assert!(error.contains("Cancelled"));
    assert!(error.contains("not rollback"));
}

#[test]
fn missing_tool_returns_an_actionable_error() {
    let directory = tempfile::tempdir().expect("create empty fixture directory");
    let path = directory.path().join("missing-cli");
    let command = process::command(&[path.to_str().expect("UTF-8 fixture path")])
        .expect("construct absent executable");
    let error = process::run(command, &AtomicBool::new(false), |_, _| Ok(()))
        .expect_err("missing tool must fail")
        .to_string();
    assert!(error.contains("required CLI is installed"));
}
