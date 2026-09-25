use super::{FakeHost, Host as _, Input, PASSWORD, SUBSCRIPTION, StoreFault, error, saved, values};
use crate::provision::run;

#[test]
fn existing_configuration_is_unchanged() {
    let mut host = FakeHost::existing();
    host.answer(&["yes"]);
    run(&mut host, None).expect("existing configuration succeeds");
    assert_eq!(host.settings("test-env"), &saved("test-env"));
    assert_eq!(host.applied, vec!["test-env".to_owned()]);
    assert!(host.imports.is_empty());
    assert!(!host.called(&["azd", "env", "new"]));
    assert!(!host.messages.join("\n").contains(PASSWORD));
    assert!(!host.called(&["gh"]));
    assert!(!host.called(&["azd", "up"]));
}

#[test]
fn empty_recipe_argument_uses_the_selected_environment() {
    let mut host = FakeHost::existing();
    host.answer(&["yes"]);
    run(&mut host, Some("")).expect("empty optional recipe argument selects the default");
    assert_eq!(host.applied, vec!["test-env".to_owned()]);
    assert!(host.imports.is_empty());
}

#[test]
fn explicit_environment_does_not_change_default() {
    let mut host = FakeHost::existing();
    host.environments.insert("second".into(), saved("second"));
    host.answer(&["yes"]);
    run(&mut host, Some("second")).expect("explicit environment succeeds");
    assert_eq!(host.applied, vec!["second".to_owned()]);
    assert_eq!(host.default, "test-env");
}

#[test]
fn process_environment_selects_explicit_target() {
    let mut host = FakeHost::existing();
    host.inputs = values(&[("AZURE_ENV_NAME", "test-env")]);
    host.answer(&["yes"]);
    run(&mut host, None).expect("process selection succeeds");
    assert_eq!(host.applied, vec!["test-env".to_owned()]);
}

#[test]
fn ambiguous_environment_requires_selection() {
    let mut host = FakeHost::existing();
    host.environments.insert("second".into(), saved("second"));
    host.default.clear();
    host.answer(&["second", "yes"]);
    run(&mut host, None).expect("chosen environment succeeds");
    assert_eq!(host.applied, vec!["second".to_owned()]);
}

#[test]
fn ambiguous_environment_without_terminal_fails() {
    let mut host = FakeHost::existing();
    host.default.clear();
    host.interactive = false;
    assert!(error(&mut host, None).contains("interactive"));
    assert!(host.applied.is_empty());
}

#[test]
fn invalid_environment_name_never_initializes() {
    for name in ["../outside", "-option", "space here", "name;echo"] {
        let mut host = FakeHost::new(None);
        assert!(error(&mut host, Some(name)).contains("environment name"));
        assert!(!host.called(&["azd", "env", "new"]));
    }
}

#[test]
fn environment_case_is_preserved() {
    let mut host = FakeHost::existing();
    assert!(error(&mut host, Some("TEST-ENV")).contains("exact name"));
    assert!(!host.called(&["azd", "env", "new"]));
}

#[test]
fn unlisted_local_configuration_is_not_reset() {
    let mut host = FakeHost::new(None);
    let directory = host.root().join(".azure/partial-env");
    std::fs::create_dir_all(&directory).expect("create partial fixture");
    let file = directory.join("config.json");
    std::fs::write(&file, "preserve this").expect("write partial fixture");
    assert!(error(&mut host, Some("partial-env")).contains("will not reinitialize"));
    assert_eq!(
        std::fs::read_to_string(file).expect("original remains"),
        "preserve this"
    );
    assert!(!host.called(&["azd", "env", "new"]));
}

#[test]
fn failed_listing_is_not_treated_as_a_missing_environment() {
    let mut host = FakeHost::new(None);
    host.fail(&["azd", "env", "list"]);
    assert!(error(&mut host, Some("new-env")).contains("CLI failure"));
    assert!(!host.called(&["azd", "env", "new"]));
}

#[test]
fn malformed_listing_is_rejected() {
    let mut host = FakeHost::new(None);
    host.override_output(
        &["azd", "env", "list", "--output", "json", "--no-prompt"],
        r#"{"not":"a list"}"#,
    );
    assert!(error(&mut host, None).contains("environment list"));
}

#[test]
fn remote_only_environment_is_not_mutated() {
    let mut host = FakeHost::new(None);
    host.override_output(
        &["azd", "env", "list", "--output", "json", "--no-prompt"],
        r#"[{"Name":"remote","IsDefault":true,"HasLocal":false}]"#,
    );
    assert!(error(&mut host, None).contains("remote-only"));
    assert!(host.applied.is_empty());
}

#[test]
fn missing_azd_capability_requires_update() {
    let mut host = FakeHost::existing();
    host.override_output(&["azd", "env", "set", "--help"], "older CLI");
    assert!(error(&mut host, None).contains("Update azd"));
    assert!(host.applied.is_empty());
}

#[test]
fn tool_failure_stops_before_setup() {
    let mut host = FakeHost::existing();
    host.fail(&["azd", "env", "set", "--help"]);
    assert!(error(&mut host, None).contains("CLI failure"));
    assert!(!host.called(&["azd", "env", "list"]));
}

#[test]
fn authentication_failures_do_not_start_login_or_provisioning() {
    for (command, guidance) in [
        (vec!["az", "account", "show"], "az login"),
        (vec!["az", "account", "get-access-token"], "az login"),
        (vec!["azd", "auth", "login"], "azd auth login"),
    ] {
        let mut host = FakeHost::existing();
        host.fail(&command);
        assert!(error(&mut host, None).contains(guidance));
        assert!(host.applied.is_empty());
        assert!(!host.called(&["az", "login"]));
    }
}

#[test]
fn disabled_subscription_is_rejected() {
    let mut host = FakeHost::existing();
    host.override_output(
        &["az", "account", "show", "--output", "json"],
        &format!(r#"{{"id":"{SUBSCRIPTION}","state":"Disabled"}}"#),
    );
    assert!(error(&mut host, None).contains("enabled subscription"));
}

#[test]
fn subscription_mismatch_never_switches_accounts() {
    let mut settings = saved("test-env");
    settings.insert(
        "AZURE_SUBSCRIPTION_ID".into(),
        "different-subscription".into(),
    );
    let mut host = FakeHost::new(Some(settings));
    assert!(error(&mut host, None).contains("account set"));
    assert!(!host.called(&["az", "account", "set"]));
    assert!(host.applied.is_empty());
}

#[test]
fn conflicting_inputs_are_reported_by_name_only() {
    let mut host = FakeHost::existing();
    host.inputs = values(&[
        ("POSTGRES_ADMIN_PASSWORD", "test-only-different-secret"),
        ("BACKEND_MAX_REPLICAS", "9"),
    ]);
    let message = error(&mut host, None);
    assert!(message.contains("POSTGRES_ADMIN_PASSWORD"));
    assert!(message.contains("BACKEND_MAX_REPLICAS"));
    assert!(!message.contains("test-only-different-secret"));
    assert_eq!(host.settings("test-env"), &saved("test-env"));
    assert!(host.applied.is_empty());
}

#[test]
fn conflicting_environment_input_is_rejected() {
    let mut host = FakeHost::existing();
    host.inputs
        .insert("AZURE_ENV_NAME".into(), "another".into());
    assert!(error(&mut host, Some("test-env")).contains("AZURE_ENV_NAME"));
}

#[test]
fn missing_secret_requires_recovery_without_a_terminal() {
    let mut settings = saved("test-env");
    settings.remove("POSTGRES_ADMIN_PASSWORD");
    let mut host = FakeHost::new(Some(settings));
    host.interactive = false;
    assert!(error(&mut host, None).contains("POSTGRES_ADMIN_PASSWORD"));
    assert!(host.imports.is_empty());
    assert!(host.applied.is_empty());
}

#[test]
fn secret_prompt_preserves_whitespace_and_punctuation() {
    let mut settings = saved("test-env");
    settings.remove("POSTGRES_ADMIN_PASSWORD");
    let mut host = FakeHost::new(Some(settings));
    let secret = " test-only 'single' \"double\" $VARIABLE ${BRACES} `echo` \\ path! \t ";
    host.answer(&[secret, "yes"]);
    run(&mut host, None).expect("secret entered and imported");
    assert_eq!(
        host.settings("test-env")
            .get("POSTGRES_ADMIN_PASSWORD")
            .map(String::as_str),
        Some(secret)
    );
    assert_eq!(
        host.prompts.first().expect("secret prompt exists").1,
        Input::Secret
    );
    assert!(!host.messages.join("\n").contains(secret));
    assert!(!format!("{:?}", host.calls).contains(secret));
    assert!(host.imports.iter().all(|(path, _)| !path.exists()));
}

#[test]
fn process_input_fills_only_a_missing_value() {
    let mut settings = saved("test-env");
    settings.remove("POSTGRES_ADMIN_PASSWORD");
    let mut host = FakeHost::new(Some(settings));
    host.inputs
        .insert("POSTGRES_ADMIN_PASSWORD".into(), PASSWORD.into());
    host.answer(&["yes"]);
    run(&mut host, None).expect("missing password imported");
    assert_eq!(host.settings("test-env"), &saved("test-env"));
    assert_eq!(host.imports.len(), 1);
    assert!(
        !host
            .imports
            .first()
            .expect("import exists")
            .1
            .contains("BACKEND_IMAGE=")
    );
}

#[test]
fn cancelling_does_not_save_missing_inputs() {
    let mut settings = saved("test-env");
    settings.remove("POSTGRES_ADMIN_PASSWORD");
    let mut host = FakeHost::new(Some(settings.clone()));
    host.inputs
        .insert("POSTGRES_ADMIN_PASSWORD".into(), PASSWORD.into());
    host.answer(&["no"]);
    assert!(error(&mut host, None).contains("Cancelled"));
    assert!(!host.prepared);
    assert!(host.imports.is_empty());
    assert!(host.applied.is_empty());
    assert_eq!(host.settings("test-env"), &settings);
}

#[test]
fn no_terminal_never_implicitly_confirms_apply() {
    let mut host = FakeHost::existing();
    host.interactive = false;
    assert!(error(&mut host, None).contains("interactive"));
    assert!(host.applied.is_empty());
}

#[test]
fn concurrent_configuration_change_stops_before_apply() {
    let mut host = FakeHost::existing();
    host.store_fault = Some(StoreFault::ConcurrentChange);
    host.answer(&["yes"]);
    assert!(error(&mut host, None).contains("changed during setup"));
    assert!(host.imports.is_empty());
    assert!(host.applied.is_empty());
}

#[test]
fn import_failure_cleans_temporary_file() {
    let mut settings = saved("test-env");
    settings.remove("POSTGRES_ADMIN_PASSWORD");
    let mut host = FakeHost::new(Some(settings));
    host.inputs
        .insert("POSTGRES_ADMIN_PASSWORD".into(), PASSWORD.into());
    host.fail(&["azd", "env", "set", "--environment"]);
    host.answer(&["yes"]);
    assert!(error(&mut host, None).contains("CLI failure"));
    let path = host
        .calls
        .iter()
        .flat_map(|(args, _)| args.windows(2))
        .find_map(|pair| match pair {
            [flag, path] if flag == "--file" => Some(path),
            _ => None,
        })
        .expect("file was passed to azd");
    assert!(!std::path::Path::new(path).exists());
    assert!(host.applied.is_empty());
}

#[test]
fn changed_imported_values_stop_before_apply() {
    let mut settings = saved("test-env");
    settings.remove("POSTGRES_ADMIN_PASSWORD");
    let mut host = FakeHost::new(Some(settings));
    host.inputs
        .insert("POSTGRES_ADMIN_PASSWORD".into(), PASSWORD.into());
    host.store_fault = Some(StoreFault::CorruptImport);
    host.answer(&["yes"]);
    assert!(error(&mut host, None).contains("preserve configuration exactly"));
    assert!(host.applied.is_empty());
}

#[test]
fn unsupported_secret_endings_never_change_the_original() {
    for suffix in ['\\', '"'] {
        let mut settings = saved("test-env");
        settings.insert(
            "POSTGRES_ADMIN_PASSWORD".into(),
            format!("{PASSWORD}{suffix}"),
        );
        let mut host = FakeHost::new(Some(settings.clone()));
        assert!(error(&mut host, None).contains("round-trip"));
        assert_eq!(host.settings("test-env"), &settings);
        assert!(host.imports.is_empty());
        assert!(host.applied.is_empty());
    }
}

#[test]
fn provisioning_failure_is_not_reported_as_success() {
    let mut host = FakeHost::existing();
    host.answer(&["yes"]);
    host.fail(&["azd", "provision"]);
    assert!(error(&mut host, None).contains("CLI failure"));
    assert!(!host.messages.join("\n").contains("provisioning completed"));
}

#[test]
fn cancelled_bootstrap_never_creates_an_environment() {
    let mut host = FakeHost::new(None);
    host.answer(&["no"]);
    assert!(error(&mut host, Some("new-env")).contains("Cancelled"));
    assert!(host.environments.is_empty());
}

#[test]
fn guided_bootstrap_does_not_write_empty_database_defaults() {
    let mut host = FakeHost::new(None);
    host.inputs = saved("new-env");
    host.inputs.remove("AZURE_RESOURCE_GROUP");
    host.inputs.remove("CUSTOM_DOMAIN_NAME");
    host.answer(&["yes", "", "", "", "", "yes", "yes"]);
    run(&mut host, Some("new-env")).expect("guided setup succeeds");
    let settings = host.settings("new-env");
    assert_eq!(
        settings.get("POSTGRES_ADMIN_PASSWORD").map(String::as_str),
        Some(PASSWORD)
    );
    assert!(!settings.contains_key("POSTGRES_ADMIN_LOGIN"));
    assert!(!settings.contains_key("POSTGRES_DB"));
    assert!(!settings.contains_key("REGISTRY_PASSWORD"));
    assert_eq!(host.applied, vec!["new-env".to_owned()]);
    assert!(host.messages.join("\n").contains("NOT encrypted"));
    assert!(!format!("{:?}", host.calls).contains(PASSWORD));
}

#[test]
fn creation_failure_stops_before_importing_or_applying() {
    let mut host = FakeHost::new(None);
    host.inputs = saved("new-env");
    host.answer(&["yes", "", "", "yes", "yes"]);
    host.fail(&["azd", "env", "new"]);
    assert!(error(&mut host, Some("new-env")).contains("CLI failure"));
    assert!(host.imports.is_empty());
    assert!(host.applied.is_empty());
}

#[test]
fn malformed_secret_values_are_not_disclosed() {
    let mut host = FakeHost::existing();
    host.override_output(
        &[
            "azd",
            "env",
            "get-values",
            "--environment",
            "test-env",
            "--output",
            "json",
            "--no-prompt",
        ],
        &format!(r#"{{"POSTGRES_ADMIN_PASSWORD":["{PASSWORD}"]}}"#),
    );
    let message = error(&mut host, None);
    assert!(message.contains("original .env"));
    assert!(!message.contains(PASSWORD));
}
