//! Namespace (publisher) page — lists all packages under a given namespace.

use html::text_content::Division;
use wasm_meta_registry_client::RegistryClient;

use crate::components::ds::package_row;
use crate::layout;

/// Render the namespace page listing all packages for a publisher.
pub(crate) async fn render(client: &RegistryClient, namespace: &str) -> String {
    match client.search_packages(namespace).await {
        Ok(packages) => {
            let filtered: Vec<_> = packages
                .iter()
                .filter(|p| p.wit_namespace.as_deref().is_some_and(|ns| ns == namespace))
                .collect();
            render_packages(namespace, &filtered)
        }
        Err(err) => {
            eprintln!("component-frontend: namespace page error for {namespace}: {err}");
            render_packages(namespace, &[])
        }
    }
}

/// Render the package listing for a namespace.
fn render_packages(
    namespace: &str,
    packages: &[&wasm_meta_registry_client::KnownPackage],
) -> String {
    let mut body = Division::builder();

    body.division(|div| {
        div.class("pt-8 pb-8")
            .heading_1(|h1| {
                h1.class(crate::components::ds::typography::H1_CLASS)
                    .text(namespace.to_owned())
            })
            .paragraph(|p| {
                p.class(crate::components::ds::typography::SUBTITLE_CLASS)
                    .text(format!(
                        "{} package{}",
                        packages.len(),
                        if packages.len() == 1 { "" } else { "s" }
                    ))
            })
    });

    if packages.is_empty() {
        body.division(|div| {
            div.class("py-16 text-center").paragraph(|p| {
                p.class("text-ink-500")
                    .text("No packages found under this namespace.")
            })
        });
    } else {
        body.division(|div| {
            div.class(package_row::HEADER_CLASS)
                .span(|s| s.class("w-48 shrink-0").text("Package"))
                .span(|s| s.class("w-20 shrink-0").text("Version"))
                .span(|s| s.text("Description"))
        });

        let mut list = Division::builder();
        list.class("divide-y divide-lineSoft");
        for pkg in packages {
            list.push(package_row::render(pkg));
        }
        body.push(list.build());
    }

    layout::document_with_nav(namespace, &body.build().to_string())
}
