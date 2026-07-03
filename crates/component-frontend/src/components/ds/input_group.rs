//! Input group — a leading dropdown button fused to a field. Picking an option
//! from the dropdown updates the trigger's label and swaps the field's value.
//! This is the design system's "Input group · button with dropdown" pattern.
//!
//! The control is self-contained: one inline script (emitted once per render,
//! but idempotent) wires up every `[data-input-group]` on the page — toggling
//! the menu, closing it on outside-click or Escape, and applying the selection.
//! Each root also exposes an `igSelect(id)` method so callers can drive the
//! selection programmatically (e.g. platform auto-detection).

use std::fmt::Write as _;

use crate::escape::{escape_html_attr, escape_html_text};

/// One choice in the dropdown menu.
pub(crate) struct DropdownOption<'a> {
    /// Stable id used in markup and for programmatic selection via `igSelect`.
    pub id: &'a str,
    /// Label shown in the menu and on the trigger button when selected.
    pub label: &'a str,
    /// Value written into the field when this option is selected.
    pub value: &'a str,
}

/// Configuration for [`button_with_dropdown`].
pub(crate) struct ButtonWithDropdown<'a> {
    /// Options in menu order. The `selected` one is shown before any input.
    pub options: &'a [DropdownOption<'a>],
    /// Index of the option shown on first render. Clamped to a valid index; an
    /// empty `options` slice renders nothing.
    pub selected: usize,
    /// Accessible name for the dropdown trigger button.
    pub trigger_aria_label: &'a str,
    /// Accessible name for the field.
    pub field_aria_label: &'a str,
    /// Extra classes appended to the field input (e.g. `"mono"`).
    pub field_class: &'a str,
    /// When true, a trailing copy button is fused to the field; clicking it
    /// copies the field's current value (which tracks the selected option).
    pub with_copy: bool,
    /// Optional static addon rendered in a bordered box between the trigger and
    /// the field (for example a `$` shell prompt). Decorative and excluded from
    /// the copied value; an empty string renders nothing.
    pub prefix: &'a str,
}

const CHEVRON: &str = concat!(
    r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">"#,
    include_str!("../../../../../vendor/lucide/chevron-down.svg"),
    "</svg>"
);
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
const COPY_BTN_CLASS: &str = "flex-none inline-flex items-center justify-center w-9 h-9 rounded-r-md border border-l-0 border-line bg-surface text-ink-500 hover:text-ink-900 hover:bg-surfaceMuted focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-ink-900";
const ADDON_CLASS: &str = "flex-none inline-flex items-center px-3 h-9 border border-r-0 border-line bg-surfaceMuted text-[13px] text-ink-500 mono select-none";

/// Behaviour for every input group on the page. Idempotent: re-running it (when
/// more than one component is rendered) skips roots it has already wired.
const SCRIPT: &str = r#"<script>
(function(){
  document.querySelectorAll('[data-input-group]').forEach(function(root){
    if (root.dataset.igReady) return;
    root.dataset.igReady = '1';
    var trigger = root.querySelector('[data-ig-trigger]');
    var menu = root.querySelector('[data-ig-menu]');
    var label = root.querySelector('[data-ig-label]');
    var field = root.querySelector('[data-ig-field]');
    var options = Array.prototype.slice.call(root.querySelectorAll('[data-ig-option]'));
    if(!trigger || !menu) return;
    function open(){ menu.hidden = false; trigger.setAttribute('aria-expanded','true'); }
    function close(){ menu.hidden = true; trigger.setAttribute('aria-expanded','false'); }
    function focusOption(i){ if(options.length){ options[Math.max(0, Math.min(i, options.length - 1))].focus(); } }
    function openToOption(){ open(); focusOption(options.findIndex(function(o){ return o.getAttribute('aria-selected') === 'true'; })); }
    function select(opt){
      if(!opt) return;
      options.forEach(function(o){ o.setAttribute('aria-selected', o === opt ? 'true' : 'false'); });
      if (label) label.textContent = opt.getAttribute('data-label');
      if (field) field.value = opt.getAttribute('data-value');
      close();
    }
    trigger.addEventListener('click', function(e){ e.stopPropagation(); if(menu.hidden){ open(); } else { close(); } });
    trigger.addEventListener('keydown', function(e){ if(e.key === 'ArrowDown' || e.key === 'ArrowUp'){ e.preventDefault(); openToOption(); } });
    options.forEach(function(o){
      o.addEventListener('click', function(e){ e.stopPropagation(); select(o); trigger.focus(); });
    });
    menu.addEventListener('keydown', function(e){
      var i = options.indexOf(document.activeElement);
      if(e.key === 'ArrowDown'){ e.preventDefault(); focusOption(i + 1); }
      else if(e.key === 'ArrowUp'){ e.preventDefault(); focusOption(i - 1); }
      else if(e.key === 'Home'){ e.preventDefault(); focusOption(0); }
      else if(e.key === 'End'){ e.preventDefault(); focusOption(options.length - 1); }
      else if(e.key === 'Tab'){ close(); }
    });
    document.addEventListener('click', function(e){ if(!root.contains(e.target)) close(); });
    document.addEventListener('keydown', function(e){ if(e.key === 'Escape' && !menu.hidden){ close(); trigger.focus(); } });
    if (field && field.hasAttribute('readonly')) {
      field.addEventListener('focus', function(){ field.select(); });
      field.addEventListener('click', function(){ field.select(); });
    }
    var copyBtn = root.querySelector('[data-ig-copy]');
    if (copyBtn && field) {
      copyBtn.addEventListener('click', function(){
        if(!navigator.clipboard) return;
        navigator.clipboard.writeText(field.value).then(function(){
          var c = copyBtn.querySelector('[data-ig-copy-icon="copy"]');
          var k = copyBtn.querySelector('[data-ig-copy-icon="check"]');
          if(c) c.hidden = true;
          if(k) k.hidden = false;
          copyBtn.setAttribute('aria-label', 'Copied');
          setTimeout(function(){
            if(c) c.hidden = false;
            if(k) k.hidden = true;
            copyBtn.setAttribute('aria-label', 'Copy command');
          }, 1500);
        });
      });
    }
    root.igSelect = function(id){ select(root.querySelector('[data-ig-option="' + id + '"]')); };
  });
})();
</script>"#;

/// Render an input group: a leading dropdown button joined to a value field.
///
/// Returns the markup plus the (idempotent) driving script. Callers that render
/// several groups can concatenate the results safely — the script wires each
/// root only once.
#[must_use]
pub(crate) fn button_with_dropdown(cfg: &ButtonWithDropdown<'_>) -> String {
    if cfg.options.is_empty() {
        return String::new();
    }
    let selected = cfg.selected.min(cfg.options.len() - 1);
    let current = cfg
        .options
        .get(selected)
        .expect("selected index is clamped to a valid option");
    let current_label = escape_html_text(current.label);
    let current_value = escape_html_attr(current.value);
    let trigger_aria = escape_html_attr(cfg.trigger_aria_label);
    let field_aria = escape_html_attr(cfg.field_aria_label);
    let field_class = cfg.field_class;

    let mut menu = String::new();
    for (i, opt) in cfg.options.iter().enumerate() {
        push_menu_item(&mut menu, opt, i == selected);
    }

    let field_round = if cfg.with_copy { "" } else { " rounded-r-md" };
    let prefix_box = if cfg.prefix.is_empty() {
        String::new()
    } else {
        format!(
            r#"
  <span aria-hidden="true" class="{ADDON_CLASS}">{prefix}</span>"#,
            prefix = escape_html_text(cfg.prefix),
        )
    };
    let copy_button = if cfg.with_copy {
        format!(
            r#"
  <button type="button" data-ig-copy aria-label="Copy command" class="{COPY_BTN_CLASS}"><span data-ig-copy-icon="copy">{COPY_ICON}</span><span data-ig-copy-icon="check" hidden class="text-positive">{CHECK_ICON}</span></button>"#
        )
    } else {
        String::new()
    };

    format!(
        r#"<div class="flex relative" data-input-group>
  <button type="button" data-ig-trigger aria-haspopup="listbox" aria-expanded="false" aria-label="{trigger_aria}" class="inline-flex items-center gap-2 px-3 h-9 rounded-l-md border border-r-0 border-line bg-surfaceMuted text-ink-700 text-[13px] hover:bg-ink-300 hover:text-ink-900 whitespace-nowrap focus:outline-none focus:border-ink-900"><span data-ig-label>{current_label}</span>{CHEVRON}</button>{prefix_box}
  <input type="text" data-ig-field readonly value="{current_value}" aria-label="{field_aria}" class="block w-full h-9 px-3{field_round} border border-line bg-surface text-[14px] text-ink-900 focus:outline-none focus:border-ink-900 {field_class}">{copy_button}
  <div data-ig-menu role="listbox" hidden class="absolute left-0 top-[calc(100%+4px)] z-20 w-max min-w-[10rem] bg-surface rounded-lg border border-line shadow-tooltip p-1">{menu}</div>
</div>{SCRIPT}"#
    )
}

/// Append one menu option button to `out`.
fn push_menu_item(out: &mut String, opt: &DropdownOption<'_>, active: bool) {
    let id = escape_html_attr(opt.id);
    let label = escape_html_text(opt.label);
    let value = escape_html_attr(opt.value);
    let selected = if active { "true" } else { "false" };
    let _ = write!(
        out,
        r#"<button type="button" role="option" data-ig-option="{id}" data-label="{label}" data-value="{value}" aria-selected="{selected}" class="flex items-center gap-2.5 px-3 h-8 rounded-md text-ink-700 text-[13px] hover:bg-surfaceMuted hover:text-ink-900 cursor-pointer w-full whitespace-nowrap">{label}</button>"#,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> String {
        button_with_dropdown(&ButtonWithDropdown {
            options: &[
                DropdownOption {
                    id: "one",
                    label: "One",
                    value: "first value",
                },
                DropdownOption {
                    id: "two",
                    label: "Two",
                    value: "second value",
                },
            ],
            selected: 1,
            trigger_aria_label: "Choose an option",
            field_aria_label: "Selected value",
            field_class: "mono",
            with_copy: false,
            prefix: "",
        })
    }

    #[test]
    fn renders_one_menu_option_per_choice() {
        let html = sample();
        assert!(html.contains(r#"data-ig-option="one""#));
        assert!(html.contains(r#"data-ig-option="two""#));
        assert!(html.contains(r#"data-value="first value""#));
        assert!(html.contains(r#"data-value="second value""#));
    }

    #[test]
    fn selected_option_drives_trigger_label_and_field_value() {
        let html = sample();
        // `selected: 1` -> the second option is shown up front.
        assert!(html.contains("<span data-ig-label>Two</span>"));
        assert!(html.contains(r#"data-ig-field readonly value="second value""#));
        assert!(html.contains(r#"data-ig-option="two" data-label="Two" data-value="second value" aria-selected="true""#));
        assert!(html.contains(r#"aria-selected="false""#));
    }

    #[test]
    fn ships_self_contained_behaviour_and_select_api() {
        let html = sample();
        assert!(html.contains("data-input-group"));
        assert!(html.contains("root.igSelect"));
        // Field carries the caller's extra classes.
        assert!(html.contains("text-ink-900 focus:outline-none focus:border-ink-900 mono"));
    }

    #[test]
    fn out_of_range_selection_is_clamped() {
        let html = button_with_dropdown(&ButtonWithDropdown {
            options: &[DropdownOption {
                id: "only",
                label: "Only",
                value: "v",
            }],
            selected: 99,
            trigger_aria_label: "x",
            field_aria_label: "y",
            field_class: "",
            with_copy: false,
            prefix: "",
        });
        assert!(html.contains("<span data-ig-label>Only</span>"));
    }

    #[test]
    fn empty_options_render_nothing() {
        let html = button_with_dropdown(&ButtonWithDropdown {
            options: &[],
            selected: 0,
            trigger_aria_label: "x",
            field_aria_label: "y",
            field_class: "",
            with_copy: false,
            prefix: "",
        });
        assert!(html.is_empty());
    }

    #[test]
    fn copy_button_is_opt_in() {
        // Default sample opts out: no copy affordance, field keeps its right radius.
        let plain = sample();
        assert!(!plain.contains(r#"data-ig-copy aria-label="Copy command""#));
        assert!(plain.contains("px-3 rounded-r-md border"));

        let copyable = button_with_dropdown(&ButtonWithDropdown {
            options: &[DropdownOption {
                id: "only",
                label: "Only",
                value: "some command",
            }],
            selected: 0,
            trigger_aria_label: "x",
            field_aria_label: "y",
            field_class: "mono",
            with_copy: true,
            prefix: "",
        });
        assert!(copyable.contains(r#"data-ig-copy aria-label="Copy command""#));
        assert!(copyable.contains(r#"data-ig-copy-icon="copy""#));
        assert!(copyable.contains(r#"data-ig-copy-icon="check""#));
        // The field surrenders its right radius to the fused copy button.
        assert!(copyable.contains("px-3 border"));
        assert!(!copyable.contains("px-3 rounded-r-md border"));
    }

    #[test]
    fn supports_keyboard_navigation() {
        // The listbox ARIA semantics are backed by real keyboard support:
        // arrow keys, Home/End, and opening straight to the selected option.
        let html = sample();
        assert!(html.contains("'ArrowDown'"));
        assert!(html.contains("'ArrowUp'"));
        assert!(html.contains("'Home'"));
        assert!(html.contains("'End'"));
        assert!(html.contains("openToOption"));
    }

    #[test]
    fn prefix_addon_is_opt_in() {
        // The default sample has no addon between the trigger and the field.
        assert!(!sample().contains(ADDON_CLASS));

        let prefixed = button_with_dropdown(&ButtonWithDropdown {
            options: &[DropdownOption {
                id: "bash",
                label: "bash",
                value: "echo hi",
            }],
            selected: 0,
            trigger_aria_label: "x",
            field_aria_label: "y",
            field_class: "mono",
            with_copy: true,
            prefix: "$",
        });
        // A decorative, bordered addon box carries the prompt between the
        // trigger and the field.
        assert!(prefixed.contains(&format!(
            r#"<span aria-hidden="true" class="{ADDON_CLASS}">$</span>"#
        )));
        // The prompt is not part of the field value that gets copied.
        assert!(prefixed.contains(r#"data-ig-field readonly value="echo hi""#));
    }
}
