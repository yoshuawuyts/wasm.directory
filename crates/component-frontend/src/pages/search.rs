//! Search results page.

// r[impl frontend.pages.search]

use html::text_content::Division;
use wasm_meta_registry_client::KnownPackage;

use crate::components::ds::results_page::{Notice, ResultsPage};
use crate::components::ds::{package_row, search_bar};
use wasm_meta_registry_client::{ApiError, RegistryClient};

/// Fetch matching packages and render the search results page.
pub(crate) async fn render(client: &RegistryClient, query: &str) -> String {
    match client.search_packages(query).await {
        Ok(packages) => render_results(query, &packages),
        Err(err) => render_error(query, &err),
    }
}

/// Render the search results.
fn render_results(query: &str, packages: &[KnownPackage]) -> String {
    let title = title(query);
    ResultsPage::new(&title)
        .document_title("Search")
        .intro(render_search_form(query))
        .rows(packages.iter().map(package_row::render))
        .empty(Notice::new("No results matched your query.").action("Browse all →", "/all"))
        .render()
}

/// Render the page with an API error message.
fn render_error(query: &str, err: &ApiError) -> String {
    let title = title(query);
    ResultsPage::new(&title)
        .document_title("Search")
        .intro(render_search_form(query))
        .render_error(&Notice::new("Unable to search").detail(err.to_string()))
}

/// The results heading; escaped when rendered.
fn title(query: &str) -> String {
    format!("Results for \u{201c}{query}\u{201d}")
}

/// Inline search form for refining queries.
fn render_search_form(query: &str) -> Division {
    search_bar::inline(query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_two_row_items_without_column_headings() {
        let packages = package_row::tests::packages();
        let html = render_results("example", &packages);
        package_row::tests::assert_listing(&html, &packages);
        assert!(html.contains("showing 4 results (total unavailable)"));
        assert!(html.contains("value=\"example\""));
    }

    #[test]
    fn queries_are_escaped_in_results_and_errors() {
        let query = "<script>x</script>";
        for html in [
            render_results(query, &[]),
            render_error(query, &ApiError::new("<b>down</b>")),
        ] {
            assert!(!html.contains("<script>x"));
            assert!(html.contains("Results for \u{201c}&lt;script&gt;x&lt;/script&gt;\u{201d}"));
        }
        assert!(render_results(query, &[]).contains("No results matched your query."));
    }
}
