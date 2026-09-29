//! Pagination presentation, independent of listing totals and link targets.

use html::text_content::Division;

/// Navigation state for an offset-paginated result page.
#[derive(Debug)]
pub(crate) struct PaginationState {
    effective_limit: u32,
    pub(crate) prev_offset: u32,
    pub(crate) next_offset: u32,
    pub(crate) has_prev: bool,
    pub(crate) has_next: bool,
    pub(crate) start: u32,
    pub(crate) end: u32,
}

impl PaginationState {
    /// Preserve the existing package listing's page-length pagination.
    #[must_use]
    pub(crate) fn new(count: usize, offset: u32, limit: u32) -> Self {
        Self::with_next(
            count,
            offset,
            limit,
            u32::try_from(count) == Ok(limit.max(1)),
        )
    }

    /// Use explicit next-page information from a relationship result page.
    #[must_use]
    pub(crate) fn with_next(count: usize, offset: u32, limit: u32, has_next: bool) -> Self {
        let effective_limit = limit.max(1);
        let next_offset = offset.saturating_add(effective_limit);
        let count = u32::try_from(count).expect("result page count should fit in u32");
        let (start, end) = if count == 0 {
            (0, 0)
        } else {
            (offset.saturating_add(1), offset.saturating_add(count))
        };
        Self {
            effective_limit,
            prev_offset: offset.saturating_sub(effective_limit),
            next_offset,
            has_prev: offset > 0,
            has_next: has_next && next_offset > offset,
            start,
            end,
        }
    }

    /// Render the range and Previous/Next controls using caller-supplied URLs.
    pub(crate) fn render(&self, href: impl Fn(u32, u32) -> String) -> Division {
        let mut container = Division::builder();
        container.class(
            "flex items-center justify-between gap-4 mt-8 pt-6 border-t-[1.5px] border-rule",
        );
        container.span(|s| {
            s.class("text-[13px] text-ink-400")
                .text(format!("Showing {}\u{2013}{}", self.start, self.end))
        });
        container.push(self.controls(&href));
        container.build()
    }

    fn controls(&self, href: &impl Fn(u32, u32) -> String) -> Division {
        let mut controls = Division::builder();
        controls.class("flex items-center gap-2");
        for (label, enabled, offset) in [
            ("Previous", self.has_prev, self.prev_offset),
            ("Next", self.has_next, self.next_offset),
        ] {
            if enabled {
                controls.anchor(|a| {
                    a.href(href(offset, self.effective_limit))
                        .class(super::breadcrumb::PAGINATION_BUTTON_CLASS)
                        .text(label)
                });
            } else {
                controls.span(|s| {
                    s.class(super::breadcrumb::PAGINATION_DISABLED_CLASS)
                        .text(label)
                });
            }
        }
        controls.build()
    }
}
