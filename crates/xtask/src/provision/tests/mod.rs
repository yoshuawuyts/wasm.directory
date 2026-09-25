//! Strict offline CLI doubles: no test invokes a real az or azd command.

mod configuration;
mod flow;
mod native;
mod safety;

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde_json::json;
use tempfile::TempDir;

use super::Values;
use super::host::{Disclosure, Host, Input};
use super::redactor::Redactor;

const SUBSCRIPTION: &str = "11111111-1111-1111-1111-111111111111";
const PASSWORD: &str = "test-only-NOT-a-real-password!$LITERAL";

#[derive(Clone, Copy, Debug)]
enum StoreFault {
    CorruptImport,
    ConcurrentChange,
}

fn values(entries: &[(&str, &str)]) -> Values {
    entries
        .iter()
        .map(|(key, value)| ((*key).into(), (*value).into()))
        .collect()
}

fn saved(name: &str) -> Values {
    values(&[
        ("AZURE_ENV_NAME", name),
        ("AZURE_SUBSCRIPTION_ID", SUBSCRIPTION),
        ("AZURE_LOCATION", "test-region"),
        ("AZURE_RESOURCE_GROUP", "rg-test-env"),
        ("POSTGRES_ADMIN_PASSWORD", PASSWORD),
        ("BACKEND_IMAGE", "ghcr.io/example/test-backend:1.2.3"),
        ("FRONTEND_IMAGE", "ghcr.io/example/test-frontend:4.5.6"),
        ("CUSTOM_DOMAIN_NAME", "registry.example"),
        ("BACKEND_MAX_REPLICAS", "4"),
        ("LOG_ANALYTICS_DAILY_QUOTA_GB", "2"),
    ])
}

#[derive(Debug)]
struct FakeHost {
    directory: TempDir,
    inputs: Values,
    environments: BTreeMap<String, Values>,
    default: String,
    answers: VecDeque<String>,
    messages: Vec<String>,
    prompts: Vec<(String, Input)>,
    calls: Vec<(Vec<String>, Disclosure)>,
    imports: Vec<(PathBuf, String)>,
    applied: Vec<String>,
    overrides: BTreeMap<Vec<String>, String>,
    failure: Option<Vec<String>>,
    interactive: bool,
    prepared: bool,
    store_fault: Option<StoreFault>,
    reads: usize,
    redactor: Redactor,
}

impl FakeHost {
    fn new(settings: Option<Values>) -> Self {
        let directory = tempfile::tempdir().expect("create fixture directory");
        std::fs::create_dir(directory.path().join("infra"))
            .expect("create fixture infra directory");
        std::fs::write(
            directory.path().join("infra/main.bicepparam"),
            include_str!("../../../../../infra/main.bicepparam"),
        )
        .expect("write fixture parameters");
        let mut environments = BTreeMap::new();
        let default = match settings {
            Some(values) => {
                let name = values
                    .get("AZURE_ENV_NAME")
                    .expect("fixture has name")
                    .clone();
                environments.insert(name.clone(), values);
                name
            }
            None => String::new(),
        };
        Self {
            directory,
            inputs: Values::new(),
            environments,
            default,
            answers: VecDeque::new(),
            messages: Vec::new(),
            prompts: Vec::new(),
            calls: Vec::new(),
            imports: Vec::new(),
            applied: Vec::new(),
            overrides: BTreeMap::new(),
            failure: None,
            interactive: true,
            prepared: false,
            store_fault: None,
            reads: 0,
            redactor: Redactor::default(),
        }
    }

    fn existing() -> Self {
        Self::new(Some(saved("test-env")))
    }

    fn answer(&mut self, answers: &[&str]) {
        self.answers = answers.iter().map(|value| (*value).to_owned()).collect();
    }

    fn override_output(&mut self, args: &[&str], output: &str) {
        self.overrides.insert(
            args.iter().map(|arg| (*arg).to_owned()).collect(),
            output.into(),
        );
    }

    fn fail(&mut self, args: &[&str]) {
        self.failure = Some(args.iter().map(|arg| (*arg).to_owned()).collect());
    }

    fn record(&mut self, args: &[&str], disclosure: Disclosure) -> Result<Vec<String>> {
        let args: Vec<_> = args.iter().map(|arg| (*arg).to_owned()).collect();
        self.calls.push((args.clone(), disclosure));
        ensure!(
            !self
                .failure
                .as_ref()
                .is_some_and(|prefix| args.starts_with(prefix)),
            "Simulated CLI failure."
        );
        Ok(args)
    }

    fn called(&self, prefix: &[&str]) -> bool {
        let prefix: Vec<_> = prefix.iter().map(|arg| (*arg).to_owned()).collect();
        self.calls.iter().any(|(args, _)| args.starts_with(&prefix))
    }

    fn settings(&self, name: &str) -> &Values {
        self.environments
            .get(name)
            .expect("fixture environment exists")
    }

    fn read_settings(&mut self, name: &str) -> Result<String> {
        self.reads += 1;
        if matches!(self.store_fault, Some(StoreFault::ConcurrentChange)) && self.reads == 2 {
            self.environments
                .get_mut(name)
                .context("fixture environment exists")?
                .insert("FRONTEND_MAX_REPLICAS".into(), "5".into());
        }
        Ok(serde_json::to_string(self.settings(name))?)
    }

    fn import(&mut self, name: &str, path: &str, disclosure: Disclosure) -> Result<String> {
        assert_eq!(disclosure, Disclosure::Private);
        let path = PathBuf::from(path);
        assert_private_file(&path);
        let text = std::fs::read_to_string(&path)?;
        self.imports.push((path, text.clone()));
        let additions = decode_import(&text)?;
        for (key, value) in additions {
            let value = if matches!(self.store_fault, Some(StoreFault::CorruptImport)) {
                "changed".into()
            } else {
                value
            };
            self.environments
                .get_mut(name)
                .context("fixture environment exists")?
                .insert(key, value);
        }
        let local = self.root().join(".azure").join(name).join(".env");
        std::fs::create_dir_all(local.parent().expect(".env has parent"))?;
        std::fs::write(local, text)?;
        Ok(String::new())
    }
}

impl Host for FakeHost {
    fn root(&self) -> &Path {
        self.directory.path()
    }

    fn inputs(&self) -> &Values {
        &self.inputs
    }

    fn capture(&mut self, args: &[&str], disclosure: Disclosure) -> Result<String> {
        let recorded = self.record(args, disclosure)?;
        if let Some(output) = self.overrides.get(&recorded) {
            return Ok(output.clone());
        }
        match args {
            ["azd", "env", "set", "--help"] => Ok("azd env set --file".into()),
            ["az", "account", "show", "--output", "json"] => {
                Ok(json!({"id": SUBSCRIPTION, "state": "Enabled"}).to_string())
            }
            [
                "az",
                "account",
                "get-access-token",
                "--query",
                "expiresOn",
                "--output",
                "tsv",
            ] => Ok("Synthetic expiry; not a token".into()),
            ["azd", "auth", "login", "--check-status"] => Ok(String::new()),
            ["azd", "env", "list", "--output", "json", "--no-prompt"] => {
                let descriptions: Vec<_> = self.environments.keys().map(|name| {
                    json!({"Name": name, "IsDefault": *name == self.default, "HasLocal": true})
                }).collect();
                Ok(serde_json::to_string(&descriptions)?)
            }
            [
                "azd",
                "env",
                "get-values",
                "--environment",
                name,
                "--output",
                "json",
                "--no-prompt",
            ] => {
                assert_eq!(disclosure, Disclosure::Private);
                self.read_settings(name)
            }
            [
                "azd",
                "env",
                "new",
                name,
                "--subscription",
                subscription,
                "--location",
                location,
                "--no-prompt",
            ] => {
                ensure!(
                    !self.environments.contains_key(*name),
                    "Environment already exists."
                );
                self.environments.insert(
                    (*name).into(),
                    values(&[
                        ("AZURE_ENV_NAME", name),
                        ("AZURE_SUBSCRIPTION_ID", subscription),
                        ("AZURE_LOCATION", location),
                    ]),
                );
                self.default = (*name).into();
                Ok(String::new())
            }
            [
                "azd",
                "env",
                "set",
                "--environment",
                name,
                "--file",
                path,
                "--no-prompt",
            ] => self.import(name, path, disclosure),
            _ => bail!("Unexpected CLI command in offline test: {args:?}"),
        }
    }

    fn prompt(&mut self, label: &str, default: &str, input: Input) -> Result<String> {
        ensure!(self.interactive, "{label} requires interactive input.");
        self.prompts.push((label.into(), input));
        let answer = self
            .answers
            .pop_front()
            .context("Terminal input cancelled.")?;
        Ok(match input {
            Input::Secret => answer,
            Input::Plain => match answer.trim() {
                "" => default.to_owned(),
                value => value.to_owned(),
            },
        })
    }

    fn message(&mut self, message: &str) -> Result<()> {
        self.messages.push(self.redactor.redact(message));
        Ok(())
    }

    fn protect(&mut self, values: &Values) {
        self.redactor.protect(values);
    }

    fn prepare_apply(&mut self) -> Result<()> {
        self.prepared = true;
        Ok(())
    }

    fn apply(&mut self, environment: &str, values: &Values) -> Result<()> {
        ensure!(self.prepared, "Applying without confirmation is forbidden.");
        assert_eq!(self.settings(environment), values);
        self.applied.push(environment.to_owned());
        self.record(
            &[
                "azd",
                "provision",
                "--environment",
                environment,
                "--no-prompt",
            ],
            Disclosure::Public,
        )?;
        Ok(())
    }
}

fn assert_private_file(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            path.metadata().expect("import exists").permissions().mode() & 0o777,
            0o600
        );
        let parent = path.parent().expect("import has private directory");
        assert_eq!(
            parent
                .metadata()
                .expect("directory exists")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    assert!(path.is_file());
}

fn decode_import(text: &str) -> Result<Values> {
    text.lines()
        .map(|line| {
            let (key, value) = line.split_once("=\"").context("quoted import")?;
            let value = value.strip_suffix('"').context("closing quote")?;
            Ok((key.to_owned(), decode_value(value)?))
        })
        .collect()
}

fn decode_value(value: &str) -> Result<String> {
    let mut output = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => output.push(match chars.next().context("escaped character")? {
                'n' => '\n',
                'r' => '\r',
                other => other,
            }),
            other => output.push(other),
        }
    }
    Ok(output)
}

fn error(host: &mut FakeHost, explicit: Option<&str>) -> String {
    format!(
        "{:#}",
        super::run(host, explicit).expect_err("flow must fail")
    )
}
