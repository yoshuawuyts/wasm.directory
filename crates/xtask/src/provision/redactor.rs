//! Credential filtering for command output and errors.

use std::collections::BTreeSet;
use std::fmt::{self, Write as _};

use super::Values;

#[derive(Default)]
pub(super) struct Redactor {
    secrets: BTreeSet<String>,
}

impl fmt::Debug for Redactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Redactor")
            .field("secret_count", &self.secrets.len())
            .finish_non_exhaustive()
    }
}

impl Redactor {
    pub(super) fn protect(&mut self, values: &Values) {
        for (key, value) in values {
            if !value.is_empty() && sensitive_key(key) {
                self.add(value);
            }
        }
    }

    fn add(&mut self, value: &str) {
        self.secrets.insert(value.into());
        self.secrets.insert(percent_encode(value));
        self.secrets
            .insert(percent_encode(value).replace("%20", "+"));
        let json = serde_json::to_string(value).expect("serializing a string cannot fail");
        let escaped = json
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .expect("JSON string has surrounding quotes");
        self.secrets.insert(escaped.to_owned());
        self.secrets.insert(ascii_json(escaped));
        let html_escaped = escaped
            .replace('&', "\\u0026")
            .replace('<', "\\u003c")
            .replace('>', "\\u003e");
        self.secrets.insert(ascii_json(&html_escaped));
        self.secrets.insert(html_escaped);
        self.secrets.extend(
            value
                .lines()
                .filter(|part| !part.is_empty())
                .map(str::to_owned),
        );
    }

    pub(super) fn redact(&self, message: &str) -> String {
        let mut secrets: Vec<_> = self.secrets.iter().collect();
        secrets.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
        secrets
            .into_iter()
            .fold(message.to_owned(), |text, secret| {
                text.replace(secret, "[redacted]")
            })
    }
}

fn sensitive_key(key: &str) -> bool {
    [
        "PASSWORD",
        "TOKEN",
        "SECRET",
        "KEY",
        "CONNECTION_STRING",
        "DATABASE_URL",
    ]
    .iter()
    .any(|marker| key.contains(marker))
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => write!(encoded, "%{byte:02X}").expect("writing to a String cannot fail"),
        }
    }
    encoded
}

fn ascii_json(escaped: &str) -> String {
    let mut output = String::new();
    for c in escaped.chars() {
        if c.is_ascii() {
            output.push(c);
        } else {
            append_unicode(&mut output, c);
        }
    }
    output
}

fn append_unicode(output: &mut String, c: char) {
    for unit in c.encode_utf16(&mut [0; 2]) {
        write!(output, "\\u{unit:04x}").expect("writing to a String cannot fail");
    }
}
