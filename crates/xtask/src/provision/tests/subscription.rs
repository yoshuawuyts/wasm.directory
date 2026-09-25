use super::{FakeHost, SUBSCRIPTION, error, saved, values};
use crate::provision::run;

#[test]
fn missing_existing_subscription_never_defaults_to_the_active_account() {
    for initial in [None, Some("")] {
        let mut settings = saved("test-env");
        settings.remove("AZURE_SUBSCRIPTION_ID");
        if let Some(value) = initial {
            settings.insert("AZURE_SUBSCRIPTION_ID".into(), value.into());
        }
        let mut host = FakeHost::new(Some(settings.clone()));
        host.answer(&["", "yes"]);
        assert!(error(&mut host, None).contains("AZURE_SUBSCRIPTION_ID is required"));
        assert_eq!(host.settings("test-env"), &settings);
        assert!(!host.prepared);
        assert!(host.imports.is_empty());
        assert!(host.applied.is_empty());
    }
}

#[test]
fn missing_existing_subscription_requires_interactive_recovery() {
    let mut settings = saved("test-env");
    settings.remove("AZURE_SUBSCRIPTION_ID");
    let mut host = FakeHost::new(Some(settings));
    host.interactive = false;
    let message = error(&mut host, None);
    assert!(message.contains("original subscription ID"));
    assert!(message.contains("interactive input"));
    assert!(host.imports.is_empty());
    assert!(host.applied.is_empty());
}

#[test]
fn existing_subscription_can_be_recovered_by_explicit_entry() {
    let mut settings = saved("test-env");
    settings.remove("AZURE_SUBSCRIPTION_ID");
    let mut host = FakeHost::new(Some(settings));
    host.answer(&[SUBSCRIPTION, "yes"]);
    run(&mut host, None).expect("explicitly recovered subscription is accepted");
    assert_eq!(host.settings("test-env"), &saved("test-env"));
    assert_eq!(host.applied, vec!["test-env".to_owned()]);
    assert!(!host.called(&["az", "account", "set"]));
}

#[test]
fn existing_subscription_can_be_recovered_from_process_configuration() {
    let mut settings = saved("test-env");
    settings.remove("AZURE_SUBSCRIPTION_ID");
    let mut host = FakeHost::new(Some(settings));
    host.inputs = values(&[("AZURE_SUBSCRIPTION_ID", SUBSCRIPTION)]);
    host.answer(&["yes"]);
    run(&mut host, None).expect("explicit process subscription is accepted");
    assert_eq!(host.settings("test-env"), &saved("test-env"));
    assert_eq!(host.prompts.len(), 1);
}

#[test]
fn new_environment_can_accept_the_active_subscription_default() {
    let mut host = FakeHost::new(None);
    host.inputs = saved("new-env");
    host.inputs.remove("AZURE_SUBSCRIPTION_ID");
    host.answer(&["yes", "", "yes", "yes"]);
    run(&mut host, Some("new-env")).expect("new setup can confirm the active subscription");
    assert_eq!(host.settings("new-env"), &saved("new-env"));
    assert_eq!(host.applied, vec!["new-env".to_owned()]);
}
