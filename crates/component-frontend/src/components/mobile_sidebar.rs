//! C06 mobile drawer, reusing the package's C01 sidebar rather than a second tree.

use html::interactive::Dialog;
use html::text_content::Division;

use super::ds::{breadcrumb::Crumb, navbar};

/// The single package sidebar shared by desktop and the mobile drawer.
pub(crate) const SIDEBAR_ID: &str = "package-sidebar";
/// Native dialog containing the mobile package navigation.
pub(crate) const DIALOG_ID: &str = "package-navigation-dialog";
/// Menu button in the mobile navbar.
pub(crate) const OPEN_ID: &str = "package-navigation-open";
/// Close button in the drawer's navbar.
pub(crate) const CLOSE_ID: &str = "package-navigation-close";

/// Render the C06 drawer shell and its progressive enhancement.
pub(crate) fn render(crumbs: &[Crumb], links: &[navbar::NavLink]) -> String {
    let panel = Division::builder()
        .class("package-navigation-panel")
        .division(|slot| slot.id("package-navigation-slot"))
        .division(|footer| {
            footer
                .class("mt-auto border-t hairline p-3")
                .push(navbar::mobile_site_links(links))
                .division(|rule| rule.class("my-2 border-t hairline"))
                .push(navbar::theme_menu_item())
        })
        .build();
    let dialog = Dialog::builder()
        .id(DIALOG_ID)
        .aria_label("Package navigation")
        .header(|header| {
            header
                .class("shrink-0 border-b hairline bg-canvas")
                .push(navbar::mobile_package_bar(crumbs, true))
        })
        .division(|body| {
            body.class("flex min-h-0 flex-1")
                .push(panel)
                .division(|scrim| {
                    scrim
                        .id("package-navigation-scrim")
                        .class("min-w-0 flex-1")
                        .aria_hidden(true)
                })
        })
        .build();
    format!("{dialog}<script>{SCRIPT}</script>")
}

/// Desktop sidebar geometry and the C06 230px drawer, using existing tokens.
pub(crate) const STYLES: &str = r"
    #package-sidebar { overscroll-behavior: contain; }
    @media (min-width: 768px) {
      #package-sidebar {
        position: sticky;
        top: var(--navbar-offset);
        max-height: calc(100vh - var(--navbar-offset));
        overflow-y: auto;
        transform: translateZ(0);
        will-change: transform;
      }
    }
    #package-navigation-dialog {
      position: fixed;
      inset: 0;
      margin: 0;
      padding: 0;
      width: 100vw;
      height: 100dvh;
      max-width: none;
      max-height: none;
      border: 0;
      background: transparent;
      color: var(--c-ink-900);
    }
    #package-navigation-dialog[open] { display: flex; flex-direction: column; }
    .package-navigation-panel {
      display: flex;
      flex-direction: column;
      flex: 0 0 230px;
      max-width: calc(100vw - 48px);
      min-height: 0;
      overflow-y: auto;
      overscroll-behavior: contain;
      background: var(--c-canvas);
      border-right: 1px solid var(--c-line);
      box-shadow: var(--shadow-card);
    }
    #package-navigation-dialog .tree-link,
    #package-navigation-dialog .tree-link > a,
    #package-navigation-dialog select,
    #package-navigation-dialog .theme-toggle { min-height: 44px; }
    #package-navigation-dialog .theme-toggle { min-width: 44px; }
    @keyframes package-navigation-enter {
      from { transform: translateX(-100%); }
      to { transform: translateX(0); }
    }
    #package-navigation-dialog[open] .package-navigation-panel {
      animation: package-navigation-enter 180ms cubic-bezier(0.2, 0, 0, 1);
    }
    @media (prefers-reduced-motion: reduce) {
      #package-navigation-dialog[open] .package-navigation-panel { animation: none; }
    }
";

const SCRIPT: &str = r"
(function() {
  var sidebar = document.getElementById('package-sidebar');
  var dialog = document.getElementById('package-navigation-dialog');
  var trigger = document.getElementById('package-navigation-open');
  var closeButton = document.getElementById('package-navigation-close');
  var slot = document.getElementById('package-navigation-slot');
  var scrim = document.getElementById('package-navigation-scrim');
  var themeButton = dialog && dialog.querySelector('.theme-toggle');
  if (!sidebar || !dialog || !trigger || !closeButton || !slot || !scrim || !themeButton) {
    console.error('Package navigation is missing a required element.');
    return;
  }
  var desktop = window.matchMedia('(min-width: 768px)');
  var marker = document.createComment('package sidebar position');
  sidebar.before(marker);
  var previousOverflow = '';

  function restore() {
    if (sidebar.parentElement !== slot) return;
    marker.after(sidebar);
    sidebar.classList.add('hidden');
    document.body.style.overflow = previousOverflow;
    trigger.setAttribute('aria-expanded', 'false');
    if (!desktop.matches) trigger.focus({ preventScroll: true });
  }
  function close() {
    dialog.close();
    restore();
  }
  function open() {
    if (desktop.matches || dialog.open) return;
    previousOverflow = document.body.style.overflow;
    // Move the existing tree so expanded groups, selection and controls survive.
    slot.append(sidebar);
    sidebar.classList.remove('hidden');
    dialog.showModal();
    document.body.style.overflow = 'hidden';
    trigger.setAttribute('aria-expanded', 'true');
    closeButton.focus({ preventScroll: true });
  }
  trigger.addEventListener('click', open);
  closeButton.addEventListener('click', close);
  scrim.addEventListener('click', close);
  dialog.addEventListener('cancel', function(event) {
    event.preventDefault();
    close();
  });
  dialog.addEventListener('close', function() {
    if (!dialog.open) restore();
  });
  dialog.addEventListener('keydown', function(event) {
    if (event.key === '/') event.stopPropagation();
    if (event.key !== 'Tab') return;
    // The close button and footer theme row bound the drawer's tab order.
    var boundary = event.shiftKey ? closeButton : themeButton;
    var destination = event.shiftKey ? themeButton : closeButton;
    if (document.activeElement === boundary) {
      event.preventDefault();
      destination.focus();
    }
  });
  dialog.addEventListener('click', function(event) {
    if (!dialog.open || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
    var link = event.target.closest('a[href]');
    if (link && link.target !== '_blank') close();
  });
  desktop.addEventListener('change', function() {
    if (desktop.matches && dialog.open) close();
  });
})();
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawer_reuses_sidebar_and_has_labeled_controls() {
        let html = render(&[], &[]);
        let opening = html.split_once('>').expect("dialog opening tag").0;
        assert!(opening.starts_with("<dialog"));
        assert!(opening.contains(r#"id="package-navigation-dialog""#));
        assert!(opening.contains(r#"aria-label="Package navigation""#));
        assert!(html.contains(r#"id="package-navigation-close""#));
        assert!(html.contains(r#"id="package-navigation-slot""#));
        assert!(!html.contains("<aside"));
        assert!(html.contains("slot.append(sidebar)"));
        assert!(html.contains("marker.after(sidebar)"));
        assert!(html.contains("dialog.addEventListener('cancel'"));
        assert!(html.contains("desktop.addEventListener('change'"));
        assert!(html.contains("event.key !== 'Tab'"));
        assert!(html.contains("event.shiftKey ? themeButton : closeButton"));
        assert!(html.contains(r#"aria-label="Site navigation""#));
        assert!(STYLES.contains("prefers-reduced-motion: reduce"));
    }
}
