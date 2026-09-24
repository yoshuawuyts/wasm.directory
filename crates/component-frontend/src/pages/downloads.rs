//! Downloads page.

use html::text_content::Division;

use crate::layout;

/// Render the downloads page with install instructions.
#[must_use]
pub(crate) fn render() -> String {
    let body = Division::builder()
        .class("pt-8 max-w-[65ch]")
        .heading_1(|h1| {
            h1.class(format!(
                "{} mb-6",
                crate::components::ds::typography::H1_CLASS
            ))
            .text("Downloads")
        })
        .paragraph(|p| {
            p.class(crate::components::ds::typography::BODY_CLASS).text(
                "Install the component CLI to manage WebAssembly components from your terminal.",
            )
        })
        .heading_2(|h2| {
            h2.class(crate::components::ds::typography::H2_CLASS)
                .text("Quick install")
        })
        .division(|d| {
            d.class("space-y-4")
                .push(install_command(
                    "Linux:",
                    "curl -fsSL https://wasm.directory/install/linux | sh",
                ))
                .push(install_command(
                    "macOS:",
                    "curl -fsSL https://wasm.directory/install/macos | sh",
                ))
                .push(install_command(
                    "Windows (PowerShell):",
                    "irm https://wasm.directory/install/windows | iex",
                ))
        })
        .heading_2(|h2| {
            h2.class(crate::components::ds::typography::H2_CLASS)
                .text("From source")
        })
        .push(
            html::text_content::PreformattedText::builder()
                .class(crate::components::ds::code::CODE_BLOCK_CLASS)
                .code(|c| c.text("cargo install component-cli"))
                .build(),
        )
        .build();

    layout::document_with_nav("Downloads", &body.to_string())
}

fn install_command(label: &'static str, command: &'static str) -> Division {
    Division::builder()
        .paragraph(|p| p.class("text-ink-700 mb-2").text(label))
        .push(
            html::text_content::PreformattedText::builder()
                .class(crate::components::ds::code::CODE_BLOCK_CLASS)
                .code(|c| c.text(command))
                .build(),
        )
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_install_uses_a_distinct_url_for_each_platform() {
        let html = render();
        for (label, command) in [
            (
                "Linux:",
                "curl -fsSL https://wasm.directory/install/linux | sh",
            ),
            (
                "macOS:",
                "curl -fsSL https://wasm.directory/install/macos | sh",
            ),
            (
                "Windows (PowerShell):",
                "irm https://wasm.directory/install/windows | iex",
            ),
        ] {
            assert!(html.contains(label), "missing platform label {label}");
            assert!(html.contains(command), "missing install command {command}");
        }
    }
}
