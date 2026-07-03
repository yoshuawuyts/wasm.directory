//! Quick-start walkthrough — a numbered list of steps that take a visitor from
//! installing the CLI to a running `wasi:http` server. Each step is a two-column
//! row: an explainer on the left and a command on the right. On narrow viewports
//! the row reflows to a single column (explainer, then command).
//!
//! Most steps render a copyable command block (`$` prefix, the command in a
//! `<code>` field, and a copy button) from [`QuickStartStep::command`]. A step
//! may instead supply arbitrary right-column markup via
//! [`QuickStartStep::command_html`] — the home page uses this to embed the
//! platform install widget as the first step. One idempotent inline script
//! wires every copy button on the page.

use std::fmt::Write as _;

use html::content::Section;

use crate::escape::{escape_html_attr, escape_html_text};

/// A single quick-start step.
pub(crate) struct QuickStartStep<'a> {
    /// Short step title shown next to the number (e.g. `"Install the CLI"`).
    pub title: &'a str,
    /// Prose explaining the step. HTML is allowed (callers may include inline
    /// `<code>` chips).
    pub body_html: &'a str,
    /// Shell command rendered as a copyable `$` block (without the `$` prompt).
    /// Ignored when `command_html` is non-empty.
    pub command: &'a str,
    /// Custom right-column markup. When non-empty it replaces the `$` command
    /// block — used to embed richer controls such as the install widget.
    pub command_html: &'a str,
}

/// Configuration for [`render`].
pub(crate) struct QuickStart<'a> {
    /// Small mono kicker above the heading.
    pub kicker: &'a str,
    /// Section heading.
    pub title: &'a str,
    /// Short intro line below the heading; HTML is allowed.
    pub intro_html: &'a str,
    /// Steps in order; each is prefixed with its 1-based number.
    pub steps: &'a [QuickStartStep<'a>],
}

const COPY_ICON: &str = concat!(
    r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">"#,
    include_str!("../../../../../vendor/lucide/copy.svg"),
    "</svg>"
);
const CHECK_ICON: &str = concat!(
    r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">"#,
    include_str!("../../../../../vendor/lucide/check.svg"),
    "</svg>"
);

const DOLLAR_CLASS: &str = "inline-flex items-center px-3 h-9 rounded-l-md border border-r-0 border-line bg-surfaceMuted text-[13px] text-ink-500 mono select-none";
const CODE_CLASS: &str = "block w-full h-9 px-3 inline-flex items-center border border-line bg-surface mono text-[13px] text-ink-900 overflow-x-auto whitespace-nowrap";
const COPY_BTN_CLASS: &str = "flex-none inline-flex items-center justify-center w-9 h-9 rounded-r-md border border-l-0 border-line bg-surface text-ink-500 hover:text-ink-900 hover:bg-surfaceMuted focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-ink-900";

/// Copy behaviour for every command block on the page. Idempotent: each button
/// is wired at most once. On success the icon flips to a check for 1.5s.
const COPY_SCRIPT: &str = r#"<script>
(function(){
  document.querySelectorAll('[data-cmd-copy]').forEach(function(btn){
    if(btn.dataset.cmdReady) return;
    btn.dataset.cmdReady = '1';
    btn.addEventListener('click', function(){
      var cmd = btn.getAttribute('data-copy');
      if(!cmd || !navigator.clipboard) return;
      navigator.clipboard.writeText(cmd).then(function(){
        var copy = btn.querySelector('[data-cmd-icon="copy"]');
        var check = btn.querySelector('[data-cmd-icon="check"]');
        if(copy) copy.hidden = true;
        if(check) check.hidden = false;
        btn.setAttribute('aria-label', 'Copied');
        setTimeout(function(){
          if(copy) copy.hidden = false;
          if(check) check.hidden = true;
          btn.setAttribute('aria-label', 'Copy command');
        }, 1500);
      });
    });
  });
})();
</script>"#;

/// Render the quick-start band.
#[must_use]
pub(crate) fn render(qs: &QuickStart<'_>) -> String {
    let kicker = qs.kicker.to_owned();
    let title = qs.title.to_owned();
    let intro = qs.intro_html.to_owned();

    let mut steps = String::new();
    for (i, step) in qs.steps.iter().enumerate() {
        push_step(&mut steps, i + 1, step);
    }
    let list = format!(r#"<ol class="mt-8" data-quickstart>{steps}</ol>{COPY_SCRIPT}"#);

    Section::builder()
        .class("mx-auto max-w-[1280px] w-full px-4 md:px-8 mt-12 md:mt-16")
        .division(|band| {
            band.class("border-t border-lineSoft pt-10 md:pt-12")
                .division(|d| {
                    d.class("text-[12px] mono uppercase tracking-wider text-ink-500")
                        .text(kicker)
                })
                .heading_3(|h| {
                    h.class("mt-2 text-[24px] font-semibold tracking-tight")
                        .text(title)
                })
                .paragraph(|p| {
                    p.class("mt-2 max-w-xl text-[13px] text-ink-700 leading-relaxed")
                        .text(intro)
                })
                .text(list)
        })
        .build()
        .to_string()
}

/// Append one numbered step to `out`: a two-column row (explainer left, command
/// right) that stacks to a single column on narrow viewports. On `md+` the left
/// explainer track is fixed at `20rem` (Tailwind's `max-w-xs`) so every row's
/// columns align and the command column absorbs the remaining width.
fn push_step(out: &mut String, n: usize, step: &QuickStartStep<'_>) {
    let title = escape_html_text(step.title);
    let body = step.body_html;
    let command = right_column(step);
    let _ = write!(
        out,
        r#"<li class="grid grid-cols-1 gap-y-4 md:grid-cols-[20rem_1fr] md:gap-x-10 md:items-start py-7 first:pt-0">
  <div class="flex gap-4">
    <span aria-hidden="true" class="flex-none flex items-center justify-center w-7 h-7 rounded-full border border-line bg-surface text-ink-900 mono text-[13px] font-medium">{n}</span>
    <div class="min-w-0">
      <h4 class="text-[15px] font-semibold tracking-tight text-ink-900">{title}</h4>
      <p class="mt-1.5 text-[13px] text-ink-700 leading-relaxed">{body}</p>
    </div>
  </div>
  <div class="min-w-0 pl-11 md:pl-0 md:max-w-[50%]">
    <h4 aria-hidden="true" class="hidden md:block invisible text-[15px] font-semibold tracking-tight">{title}</h4>
    <div class="md:mt-1.5">{command}</div>
  </div>
</li>"#,
    );
}

/// Build the right-column content for a step: either the caller's custom markup
/// or a copyable `$` command block.
fn right_column(step: &QuickStartStep<'_>) -> String {
    if !step.command_html.is_empty() {
        return step.command_html.to_owned();
    }
    let command_text = escape_html_text(step.command);
    let command_attr = escape_html_attr(step.command);
    format!(
        r#"<div class="flex">
      <span aria-hidden="true" class="{DOLLAR_CLASS}">$</span>
      <code class="{CODE_CLASS}">{command_text}</code>
      <button type="button" data-cmd-copy data-copy="{command_attr}" aria-label="Copy command" class="{COPY_BTN_CLASS}"><span data-cmd-icon="copy">{COPY_ICON}</span><span data-cmd-icon="check" hidden class="text-positive">{CHECK_ICON}</span></button>
    </div>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> String {
        render(&QuickStart {
            kicker: "Quick start",
            title: "Run your first component",
            intro_html: "Install the CLI, then reach a running server.",
            steps: &[
                QuickStartStep {
                    title: "Install the CLI",
                    body_html: "Grab it for your platform.",
                    command: "",
                    command_html: r#"<div data-install-cta>PLATFORM PICKER</div>"#,
                },
                QuickStartStep {
                    title: "Initialize a project",
                    body_html: "Scaffold a <code>wasm.toml</code>.",
                    command: "component init",
                    command_html: "",
                },
                QuickStartStep {
                    title: "Add a dependency",
                    body_html: "Install a <code>wasi:http</code> server.",
                    command: "component install ba:sample-wasi-http-rust",
                    command_html: "",
                },
            ],
        })
    }

    #[test]
    fn renders_a_numbered_step_per_entry() {
        let html = sample();
        assert!(html.contains("data-quickstart"));
        for n in ["1", "2", "3"] {
            assert!(
                html.contains(&format!(">{n}</span>")),
                "missing step number {n}"
            );
        }
        // Step titles render.
        assert!(html.contains(">Install the CLI</h4>"));
        assert!(html.contains(">Initialize a project</h4>"));
    }

    #[test]
    fn shell_steps_render_a_copyable_command_block() {
        let html = sample();
        for cmd in [
            "component init",
            "component install ba:sample-wasi-http-rust",
        ] {
            assert!(
                html.contains(&format!(">{cmd}</code>")),
                "missing command {cmd}"
            );
            assert!(
                html.contains(&format!(r#"data-copy="{cmd}""#)),
                "command {cmd} is not copyable"
            );
        }
        assert!(html.contains("data-cmd-copy"));
        assert!(html.contains("navigator.clipboard"));
    }

    #[test]
    fn custom_command_html_replaces_the_shell_block() {
        let html = sample();
        // Step one uses custom right-column markup verbatim...
        assert!(html.contains(r#"<div data-install-cta>PLATFORM PICKER</div>"#));
        // ...and does not wrap it in a `$` command block.
        assert!(!html.contains("<code class=\"block w-full h-9 px-3 inline-flex items-center border border-line bg-surface mono text-[13px] text-ink-900 overflow-x-auto whitespace-nowrap\"></code>"));
    }

    #[test]
    fn rows_use_a_responsive_two_column_grid() {
        let html = sample();
        assert!(html.contains("grid-cols-1"));
        assert!(html.contains("md:grid-cols-[20rem_1fr]"));
        // Command column halves its width on md+ and, when stacked on narrow
        // screens, indents to line up with the step text (not the number).
        assert!(html.contains("pl-11 md:pl-0 md:max-w-[50%]"));
        // On md+, an invisible title-sized spacer drops the command down so it
        // starts level with the paragraph, below the section title.
        assert!(html.contains(r#"aria-hidden="true" class="hidden md:block invisible text-[15px] font-semibold tracking-tight""#));
    }

    #[test]
    fn prose_html_is_rendered_verbatim() {
        let html = sample();
        assert!(html.contains("Install a <code>wasi:http</code> server."));
    }
}
