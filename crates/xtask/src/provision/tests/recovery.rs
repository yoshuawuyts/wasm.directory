use super::{FakeHost, error, saved, values};
use crate::provision::run;

const OVERRIDES: [&str; 4] = [
    "AZURE_RESOURCE_GROUP",
    "POSTGRES_ADMIN_LOGIN",
    "POSTGRES_DB",
    "CUSTOM_DOMAIN_NAME",
];

#[test]
fn existing_missing_overrides_require_original_values() {
    for key in OVERRIDES {
        let original = saved("test-env");
        let recovered = original
            .get(key)
            .expect("fixture has the original override");
        let mut partial = original.clone();
        partial.remove(key);
        let mut host = FakeHost::new(Some(partial));
        host.answer(&[recovered, "yes"]);
        run(&mut host, None).expect("explicitly recovered override is accepted");
        assert_eq!(host.settings("test-env"), &original);
        assert_eq!(host.applied, vec!["test-env".to_owned()]);
        assert!(
            host.prompts
                .first()
                .expect("recovery prompt")
                .0
                .contains(key)
        );
        assert!(!host.called(&["azd", "env", "new"]));
    }
}

#[test]
fn process_inputs_can_recover_missing_overrides_without_reprompting() {
    for key in OVERRIDES {
        let original = saved("test-env");
        let recovered = original
            .get(key)
            .expect("fixture has the original override");
        let mut partial = original.clone();
        partial.remove(key);
        let mut host = FakeHost::new(Some(partial));
        host.inputs = values(&[(key, recovered)]);
        host.answer(&["yes"]);
        run(&mut host, None).expect("explicit process override is accepted");
        assert_eq!(host.settings("test-env"), &original);
        assert_eq!(host.prompts.len(), 1);
    }
}

#[test]
fn missing_overrides_without_a_terminal_never_assume_defaults() {
    for key in OVERRIDES {
        let mut partial = saved("test-env");
        partial.remove(key);
        let mut host = FakeHost::new(Some(partial.clone()));
        host.interactive = false;
        let message = error(&mut host, None);
        assert!(message.contains(key));
        assert!(message.contains("interactive input"));
        assert_eq!(host.settings("test-env"), &partial);
        assert!(host.imports.is_empty());
        assert!(host.applied.is_empty());
    }
}

#[test]
fn confirmed_defaults_remain_unset_instead_of_becoming_empty_values() {
    for key in OVERRIDES {
        for incoming in [None, Some("")] {
            let mut partial = saved("test-env");
            partial.remove(key);
            let mut host = FakeHost::new(Some(partial.clone()));
            if let Some(value) = incoming {
                host.inputs.insert(key.into(), value.into());
            }
            host.answer(&["", "yes", "yes"]);
            run(&mut host, None).expect("explicitly confirmed original default is accepted");
            assert_eq!(host.settings("test-env"), &partial);
            assert!(host.imports.is_empty());
            assert_eq!(host.applied, vec!["test-env".to_owned()]);
            assert!(host.prompts.iter().any(|(label, _)| {
                label.starts_with("Use the Bicep default") && label.contains(key)
            }));
        }
    }
}

#[test]
fn rejecting_or_omitting_default_confirmation_stops_without_changes() {
    for key in OVERRIDES {
        for answer in ["no", ""] {
            let mut partial = saved("test-env");
            partial.remove(key);
            let mut host = FakeHost::new(Some(partial.clone()));
            host.answer(&["", answer]);
            assert!(error(&mut host, None).contains("Cancelled"));
            assert_eq!(host.settings("test-env"), &partial);
            assert!(!host.prepared);
            assert!(host.imports.is_empty());
            assert!(host.applied.is_empty());
        }
    }
}

#[test]
fn cancelling_apply_does_not_persist_recovered_overrides() {
    for key in OVERRIDES {
        let original = saved("test-env");
        let recovered = original
            .get(key)
            .expect("fixture has the original override");
        let mut partial = original.clone();
        partial.remove(key);
        let mut host = FakeHost::new(Some(partial.clone()));
        host.answer(&[recovered, "no"]);
        assert!(error(&mut host, None).contains("Cancelled"));
        assert_eq!(host.settings("test-env"), &partial);
        assert!(host.imports.is_empty());
        assert!(host.applied.is_empty());
    }
}

#[test]
fn explicit_empty_resource_group_and_domain_settings_are_preserved() {
    let mut original = saved("test-env");
    original.insert("AZURE_RESOURCE_GROUP".into(), String::new());
    original.insert("CUSTOM_DOMAIN_NAME".into(), String::new());
    let mut host = FakeHost::new(Some(original.clone()));
    host.answer(&["yes"]);
    run(&mut host, None).expect("explicitly configured empty overrides are preserved");
    assert_eq!(host.settings("test-env"), &original);
    assert!(host.imports.is_empty());
    assert_eq!(host.prompts.len(), 1);
}
