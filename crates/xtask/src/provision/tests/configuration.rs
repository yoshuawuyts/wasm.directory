use super::{FakeHost, Input, PASSWORD, error, saved, values};
use crate::provision::run;

#[test]
fn public_ghcr_images_need_no_registry_credentials() {
    let mut host = FakeHost::existing();
    host.answer(&["yes"]);
    run(&mut host, None).expect("public images succeed");
    assert!(!host.settings("test-env").contains_key("REGISTRY_PASSWORD"));
    assert!(host.prompts.iter().all(|(_, input)| *input == Input::Plain));
}

#[test]
fn private_registry_credentials_are_preserved() {
    let mut settings = saved("test-env");
    settings.extend(values(&[
        ("REGISTRY_SERVER", "ghcr.io"),
        ("REGISTRY_USERNAME", "example"),
        ("REGISTRY_PASSWORD", "test-only-pull-token"),
    ]));
    let mut host = FakeHost::new(Some(settings.clone()));
    host.answer(&["yes"]);
    run(&mut host, None).expect("private credentials reused");
    assert_eq!(host.settings("test-env"), &settings);
    assert!(host.imports.is_empty());
    assert!(!host.messages.join("\n").contains("test-only-pull-token"));
}

#[test]
fn private_registry_requires_missing_credentials() {
    let mut settings = saved("test-env");
    settings.extend(values(&[
        ("REGISTRY_SERVER", "ghcr.io"),
        ("REGISTRY_USERNAME", "example"),
    ]));
    let mut host = FakeHost::new(Some(settings));
    host.interactive = false;
    assert!(error(&mut host, None).contains("REGISTRY_PASSWORD"));
    assert!(host.applied.is_empty());
}

#[test]
fn registry_password_prompt_is_hidden() {
    let mut settings = saved("test-env");
    settings.extend(values(&[
        ("REGISTRY_SERVER", "ghcr.io"),
        ("REGISTRY_USERNAME", "example"),
    ]));
    let mut host = FakeHost::new(Some(settings));
    host.answer(&["test-only-pull-token", "yes"]);
    run(&mut host, None).expect("missing pull token imported");
    assert_eq!(
        host.prompts.first().expect("secret prompt").1,
        Input::Secret
    );
    assert_eq!(
        host.settings("test-env")
            .get("REGISTRY_PASSWORD")
            .map(String::as_str),
        Some("test-only-pull-token")
    );
    assert!(!format!("{:?}", host.calls).contains("test-only-pull-token"));
}

#[test]
fn unmatched_registry_does_not_discard_credentials() {
    let mut settings = saved("test-env");
    settings.extend(values(&[
        ("REGISTRY_SERVER", "different.example"),
        ("REGISTRY_USERNAME", "example"),
        ("REGISTRY_PASSWORD", "test-only-pull-token"),
    ]));
    let mut host = FakeHost::new(Some(settings.clone()));
    assert!(error(&mut host, None).contains("does not match"));
    assert_eq!(host.settings("test-env"), &settings);
}

#[test]
fn orphan_credentials_require_registry_recovery() {
    let mut settings = saved("test-env");
    settings.extend(values(&[
        ("REGISTRY_USERNAME", "example"),
        ("REGISTRY_PASSWORD", "test-only-token"),
    ]));
    let mut host = FakeHost::new(Some(settings));
    host.interactive = false;
    assert!(error(&mut host, None).contains("REGISTRY_SERVER"));
}

#[test]
fn ci_pull_token_is_not_automatically_applied() {
    let mut host = FakeHost::existing();
    host.inputs
        .insert("GHCR_PULL_TOKEN".into(), "test-only-ci-token".into());
    host.answer(&["yes"]);
    run(&mut host, None).expect("public images ignore CI token");
    assert!(!host.settings("test-env").contains_key("REGISTRY_PASSWORD"));
}

#[test]
fn missing_image_never_falls_back_to_a_demo_image() {
    let mut settings = saved("test-env");
    settings.remove("BACKEND_IMAGE");
    let mut host = FakeHost::new(Some(settings));
    host.interactive = false;
    assert!(error(&mut host, None).contains("BACKEND_IMAGE"));
    assert!(host.applied.is_empty());
}

#[test]
fn demo_images_are_rejected_even_with_digests() {
    for suffix in [":latest".to_owned(), format!("@sha256:{}", "a".repeat(64))] {
        let mut settings = saved("test-env");
        settings.insert(
            "BACKEND_IMAGE".into(),
            format!("mcr.microsoft.com/azuredocs/containerapps-helloworld{suffix}"),
        );
        let mut host = FakeHost::new(Some(settings));
        assert!(error(&mut host, None).contains("demo image"));
        assert!(host.applied.is_empty());
    }
}

#[test]
fn explicit_mutable_images_require_extra_confirmation() {
    for image in [
        "ghcr.io/example/backend:latest",
        "localhost:5000/example/backend",
    ] {
        let mut settings = saved("test-env");
        settings.insert("BACKEND_IMAGE".into(), image.into());
        let mut host = FakeHost::new(Some(settings));
        host.answer(&["yes", "yes"]);
        run(&mut host, None).expect("explicit mutable image approved");
        assert_eq!(
            host.settings("test-env")
                .get("BACKEND_IMAGE")
                .map(String::as_str),
            Some(image)
        );
        assert_eq!(host.applied, vec!["test-env".to_owned()]);
    }
}

#[test]
fn rejecting_a_mutable_image_never_changes_it() {
    let mut settings = saved("test-env");
    settings.insert(
        "BACKEND_IMAGE".into(),
        "ghcr.io/example/backend:latest".into(),
    );
    let mut host = FakeHost::new(Some(settings.clone()));
    host.answer(&["no"]);
    assert!(error(&mut host, None).contains("Cancelled"));
    assert_eq!(host.settings("test-env"), &settings);
    assert!(host.applied.is_empty());
}

#[test]
fn digest_pinning_needs_only_apply_confirmation() {
    let mut settings = saved("test-env");
    settings.insert(
        "BACKEND_IMAGE".into(),
        format!("ghcr.io/example/backend:latest@sha256:{}", "a".repeat(64)),
    );
    let mut host = FakeHost::new(Some(settings));
    host.answer(&["yes"]);
    run(&mut host, None).expect("digest-pinned image succeeds");
}

#[test]
fn invalid_image_references_do_not_echo_credentials() {
    for image in [
        "https://user:test-only-secret@registry.example/image",
        "ghcr.io/example/backend@invalid-digest",
        "<image-placeholder>",
    ] {
        let mut settings = saved("test-env");
        settings.insert("BACKEND_IMAGE".into(), image.into());
        let mut host = FakeHost::new(Some(settings));
        let message = error(&mut host, None);
        assert!(message.contains("image reference"));
        assert!(!message.contains("test-only-secret"));
    }
}

#[test]
fn empty_optional_database_settings_do_not_override_defaults() {
    for key in ["POSTGRES_ADMIN_LOGIN", "POSTGRES_DB"] {
        let mut settings = saved("test-env");
        settings.insert(key.into(), String::new());
        let mut host = FakeHost::new(Some(settings));
        assert!(error(&mut host, None).contains("explicitly empty"));
        assert!(host.applied.is_empty());
    }
}

#[test]
fn malformed_registry_hosts_are_rejected() {
    for server in ["https://ghcr.io", "ghcr.io:abc", "ghcr.io:", "ghcr.io/path"] {
        let mut settings = saved("test-env");
        settings.insert("REGISTRY_SERVER".into(), server.into());
        let mut host = FakeHost::new(Some(settings));
        assert!(error(&mut host, None).contains("hostname"));
    }
}

#[test]
fn declined_image_visibility_does_not_guess_public_access() {
    let mut settings = saved("test-env");
    settings.remove("BACKEND_IMAGE");
    let mut host = FakeHost::new(Some(settings));
    host.inputs
        .insert("BACKEND_IMAGE".into(), "ghcr.io/example/private:1.0".into());
    host.answer(&[""]);
    assert!(error(&mut host, None).contains("visibility"));
}

#[test]
fn empty_password_is_not_generated() {
    let mut settings = saved("test-env");
    settings.remove("POSTGRES_ADMIN_PASSWORD");
    let mut host = FakeHost::new(Some(settings));
    host.answer(&[""]);
    assert!(error(&mut host, None).contains("no value was generated"));
    assert!(host.imports.is_empty());
    assert!(host.applied.is_empty());
}

#[test]
fn nul_credentials_are_rejected_without_disclosure() {
    let mut settings = saved("test-env");
    settings.insert("POSTGRES_ADMIN_PASSWORD".into(), format!("{PASSWORD}\0"));
    let mut host = FakeHost::new(Some(settings));
    let message = error(&mut host, None);
    assert!(message.contains("NUL"));
    assert!(!message.contains(PASSWORD));
}
