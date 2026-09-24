use super::*;

#[cfg(not(all(target_os = "wasi", target_env = "p2")))]
mod requests;

#[test]
fn renders_flowing_rows_without_column_headings() {
    let packages = package_row::tests::packages();
    let html = render_packages(&packages, Some(245), 0, 100);
    package_row::tests::assert_listing(&html, &packages);
    assert!(html.contains("All Packages"));
    assert!(html.contains("showing 4 of 245 results"));
    assert!(html.contains("Showing 1\u{2013}4"));
}

#[test]
fn summaries_preserve_page_counts_and_registry_totals() {
    for (page_count, total, expected) in [
        (100, Some(245), "showing 100 of 245 results"),
        (45, Some(245), "showing 45 of 245 results"),
        (1, Some(1), "showing 1 of 1 result"),
        (1, Some(245), "showing 1 of 245 results"),
        (0, Some(0), "showing 0 of 0 results"),
        (0, Some(245), "showing 0 of 245 results"),
        (
            100,
            Some(u64::MAX),
            "showing 100 of 18446744073709551615 results",
        ),
        (4, None, "showing 4 results (total unavailable)"),
        (1, None, "showing 1 result (total unavailable)"),
        (0, None, "showing 0 results (total unavailable)"),
        (4, Some(3), "showing 4 results (total unavailable)"),
    ] {
        assert_eq!(result_summary(page_count, total), expected);
    }
}

#[test]
fn page_summaries_use_page_length_not_limit_or_offset() {
    let package = package_row::tests::packages().remove(0);
    for (count, offset, expected) in [
        (100, 0, "showing 100 of 245 results"),
        (100, 100, "showing 100 of 245 results"),
        (45, 200, "showing 45 of 245 results"),
        (0, 300, "showing 0 of 245 results"),
    ] {
        let html = render_packages(&vec![package.clone(); count], Some(245), offset, 100);
        assert!(html.contains(expected));
    }
}

#[test]
fn empty_registry_keeps_empty_state_and_reports_zero() {
    let html = render_packages(&[], Some(0), 0, 100);
    assert!(html.contains("showing 0 of 0 results"));
    assert!(html.contains("No packages found."));
}

#[test]
fn unavailable_or_stale_totals_preserve_rows_and_pagination() {
    let packages = package_row::tests::packages();
    for total in [None, Some(3)] {
        let html = render_packages(&packages, total, 4, 4);
        package_row::tests::assert_listing(&html, &packages);
        assert!(html.contains("showing 4 results (total unavailable)"));
        assert!(html.contains("href=\"/all?offset=0&limit=4\""));
        assert!(html.contains("href=\"/all?offset=8&limit=4\""));
        assert!(!html.contains("Unable to load packages"));
    }
}

#[test]
fn total_does_not_change_repository_offset_pagination() {
    let packages = package_row::tests::packages();
    let html = render_packages(&packages, Some(4), 400, 4);
    assert!(html.contains("showing 4 of 4 results"));
    assert!(html.contains("href=\"/all?offset=396&limit=4\""));
    assert!(html.contains("href=\"/all?offset=404&limit=4\""));
}

#[test]
fn header_with_long_total_snapshot() {
    insta::assert_snapshot!(render_header(100, Some(u64::MAX)).to_string());
}

#[test]
fn header_with_unavailable_total_snapshot() {
    insta::assert_snapshot!(render_header(100, None).to_string());
}

// r[verify frontend.pages.all]
#[test]
fn pagination_state_calculates_prev_and_next_offsets() {
    const PAGE_SIZE: u32 = 100;
    const SECOND_PAGE_OFFSET: u32 = 100;
    let state = PaginationState::new(100, SECOND_PAGE_OFFSET, PAGE_SIZE);
    assert_eq!(state.prev_offset, 0);
    assert_eq!(state.next_offset, 200);
    assert!(state.has_prev);
    assert!(state.has_next);
    assert_eq!(state.start, 101);
    assert_eq!(state.end, 200);
}
