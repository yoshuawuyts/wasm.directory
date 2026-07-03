//! Install widget — the platform picker for installing the `component` CLI.
//! A leading dropdown selects the platform (Linux, macOS, Windows) and the
//! joined field shows the matching install command. Inline JS auto-detects the
//! visitor's platform on load and pre-selects it; without JS the first option
//! stays selected and its command remains visible.
//!
//! It renders just the control (no heading or band) so it can be embedded — for
//! example as the first step of the home page quick-start walkthrough.

use crate::components::ds::input_group::{self, ButtonWithDropdown, DropdownOption};

/// A single platform's install instructions.
pub(crate) struct InstallOption<'a> {
    /// Slug used in markup and platform auto-detection: `"linux"`, `"macos"`,
    /// or `"windows"`.
    pub id: &'a str,
    /// Dropdown label shown to the user (e.g. `"Linux"`).
    pub label: &'a str,
    /// Shell command that installs the CLI on this platform.
    pub command: &'a str,
}

/// One-shot platform auto-detection. Scoped to the install widget so it is a
/// no-op on pages without it, it maps the visitor's platform to an option id
/// and drives the dropdown through the input group's `igSelect` API.
const AUTODETECT_SCRIPT: &str = r"<script>
(function(){
  var root = document.querySelector('[data-install-cta] [data-input-group]');
  if(!root || typeof root.igSelect !== 'function') return;
  var ua = ((navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || navigator.userAgent || '').toLowerCase();
  var id = null;
  if (ua.indexOf('win') !== -1) id = 'windows';
  else if (ua.indexOf('mac') !== -1) id = 'macos';
  else if (ua.indexOf('linux') !== -1 || ua.indexOf('x11') !== -1) id = 'linux';
  if (id) root.igSelect(id);
})();
</script>";

/// Render the platform install widget: a dropdown joined to the command field,
/// plus the platform auto-detection script.
#[must_use]
pub(crate) fn widget(options: &[InstallOption<'_>]) -> String {
    let choices: Vec<DropdownOption<'_>> = options
        .iter()
        .map(|o| DropdownOption {
            id: o.id,
            label: o.label,
            value: o.command,
        })
        .collect();

    let group = input_group::button_with_dropdown(&ButtonWithDropdown {
        options: &choices,
        selected: 0,
        trigger_aria_label: "Choose your platform",
        field_aria_label: "Install command",
        field_class: "mono",
        with_copy: true,
        prefix: "$",
    });

    format!(r#"<div class="min-w-0" data-install-cta>{group}</div>{AUTODETECT_SCRIPT}"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> String {
        widget(&[
            InstallOption {
                id: "linux",
                label: "Linux",
                command: "curl -LsSf https://example.com/install.sh | sh",
            },
            InstallOption {
                id: "macos",
                label: "macOS",
                command: "curl -LsSf https://example.com/install.sh | sh",
            },
            InstallOption {
                id: "windows",
                label: "Windows",
                command: "irm https://example.com/install.ps1 | iex",
            },
        ])
    }

    #[test]
    fn renders_a_dropdown_option_per_platform() {
        let html = sample();
        for id in ["linux", "macos", "windows"] {
            assert!(
                html.contains(&format!(r#"data-ig-option="{id}""#)),
                "missing dropdown option for {id}"
            );
        }
        assert!(html.contains(">Linux<") && html.contains(">macOS<") && html.contains(">Windows<"));
    }

    #[test]
    fn linux_is_selected_before_js_runs() {
        let html = sample();
        // The first option is pre-selected server-side; auto-detect corrects it.
        assert!(html.contains(r#"data-ig-option="linux" data-label="Linux" data-value="curl -LsSf https://example.com/install.sh | sh" aria-selected="true""#));
        assert!(html.contains(r#"data-ig-option="macos" data-label="macOS" data-value="curl -LsSf https://example.com/install.sh | sh" aria-selected="false""#));
        // The command field shows the selected platform's command up front.
        assert!(html.contains(
            r#"data-ig-field readonly value="curl -LsSf https://example.com/install.sh | sh""#
        ));
    }

    #[test]
    fn each_platform_command_is_present() {
        let html = sample();
        assert!(html.contains("curl -LsSf https://example.com/install.sh | sh"));
        assert!(html.contains("irm https://example.com/install.ps1 | iex"));
    }

    #[test]
    fn includes_platform_autodetect_script() {
        let html = sample();
        assert!(html.contains("data-install-cta"));
        assert!(html.contains("navigator.userAgentData"));
        assert!(html.contains("navigator.platform"));
        assert!(html.contains("root.igSelect"));
        // Detects each supported platform.
        assert!(html.contains("'windows'") && html.contains("'macos'") && html.contains("'linux'"));
    }

    #[test]
    fn shows_a_dollar_prompt_addon() {
        let html = sample();
        // A decorative `$` prompt sits between the platform picker and the
        // command field.
        assert!(html.contains(r#"aria-hidden="true""#));
        assert!(html.contains(">$</span>"));
    }
}
