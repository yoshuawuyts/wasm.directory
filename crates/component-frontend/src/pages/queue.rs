//! Status page — shows the fetch queue with tabbed Queue / History views.

use std::fmt::Write;

use html::text_content::Division;
use wasm_meta_registry_client::{QueueStatus, QueueTask, RegistryClient};

use crate::components::ds::typography;
use crate::layout;

/// Render the status page.
pub(crate) async fn render(client: &RegistryClient) -> String {
    match client.fetch_queue_status().await {
        Ok(status) => render_status(&status),
        Err(err) => render_error(&err.to_string()),
    }
}

/// Render the status overview with tabbed Queue / History views.
fn render_status(status: &QueueStatus) -> String {
    let mut body = Division::builder();
    body.class("pt-8 pb-12 max-w-4xl space-y-8");

    // Heading
    body.heading_1(|h1| {
        h1.class(format!("{} mb-2", typography::H1_CLASS))
            .text("Status")
    });
    body.paragraph(|p| {
        p.class(typography::BODY_CLASS).text(
            "Background task queue for pulling package versions from OCI registries \
             and reindexing cached data.",
        )
    });

    // Status summary cards
    body.division(|cards| {
        cards
            .class("grid grid-cols-2 sm:grid-cols-4 gap-4 mt-6")
            .push(status_card("Pending", status.pending, "text-amber-600"))
            .push(status_card(
                "In Progress",
                status.in_progress,
                "text-blue-600",
            ))
            .push(status_card("Completed", status.completed, "text-positive"))
            .push(status_card("Failed", status.failed, "text-negative"))
    });

    // Tab bar + tab panels
    let active_count = status.active.len();
    let history_count = status.history.len();
    body.text(render_tabs(
        &status.active,
        &status.history,
        active_count,
        history_count,
    ));

    // Auto-refresh while tasks are active.
    let has_active = status.pending > 0 || status.in_progress > 0;
    let script = if has_active {
        "<script>setTimeout(function(){location.reload()},30000)</script>"
    } else {
        ""
    };

    let html = body.build().to_string();
    layout::document_with_nav("Status", &format!("{html}{script}"))
}

/// Render a single status summary card.
fn status_card(label: &str, count: u64, color_class: &str) -> Division {
    Division::builder()
        .class("border border-line rounded-lg p-4 bg-surface")
        .division(|d| {
            d.class(format!("text-[28px] font-semibold {color_class}"))
                .text(count.to_string())
        })
        .division(|d| {
            d.class("text-[13px] text-ink-500 mt-1")
                .text(label.to_owned())
        })
        .build()
}

/// Render the tabbed section with Queue and History panels.
fn render_tabs(
    active: &[QueueTask],
    history: &[QueueTask],
    active_count: usize,
    history_count: usize,
) -> String {
    let tab_script = r"<script>
document.querySelectorAll('[data-tab]').forEach(function(btn){
  btn.addEventListener('click',function(){
    var target=btn.getAttribute('data-tab');
    document.querySelectorAll('[data-tab]').forEach(function(b){
      b.classList.remove('border-ink-900','text-ink-900');
      b.classList.add('border-transparent','text-ink-500');
    });
    btn.classList.remove('border-transparent','text-ink-500');
    btn.classList.add('border-ink-900','text-ink-900');
    document.querySelectorAll('[data-panel]').forEach(function(p){
      p.style.display=p.getAttribute('data-panel')===target?'block':'none';
    });
  });
});
</script>";

    let queue_panel = if active.is_empty() {
        "<p class=\"text-ink-500 text-[13px] py-6\">No active tasks.</p>".to_owned()
    } else {
        render_table_html(active, true)
    };

    let history_panel = if history.is_empty() {
        "<p class=\"text-ink-500 text-[13px] py-6\">No completed or failed tasks yet.</p>"
            .to_owned()
    } else {
        render_table_html(history, false)
    };

    format!(
        "<div class=\"mt-8\">\
           <div class=\"flex border-b border-line gap-6\">\
             <button data-tab=\"queue\" class=\"pb-2 text-[13px] font-medium border-b-2 \
               border-ink-900 text-ink-900 cursor-pointer\">\
               Queue <span class=\"ml-1 text-ink-400\">({active_count})</span>\
             </button>\
             <button data-tab=\"history\" class=\"pb-2 text-[13px] font-medium border-b-2 \
               border-transparent text-ink-500 hover:text-ink-700 cursor-pointer\">\
               History <span class=\"ml-1 text-ink-400\">({history_count})</span>\
             </button>\
           </div>\
           <div data-panel=\"queue\" class=\"pt-4\">{queue_panel}</div>\
           <div data-panel=\"history\" class=\"pt-4\" style=\"display:none\">{history_panel}</div>\
         </div>\
         {tab_script}"
    )
}

/// Render a task table as raw HTML.
///
/// When `show_priority` is true, shows a Priority column (active queue).
/// When false, shows a Completed-at column (history).
fn render_table_html(tasks: &[QueueTask], show_priority: bool) -> String {
    let mut rows = String::new();
    for task in tasks {
        let badge = status_badge_html(&task.status);

        let error_cell = task
            .last_error
            .as_deref()
            .map(|e: &str| {
                let truncated = if e.len() > 80 {
                    format!("{}…", &e[..80])
                } else {
                    e.to_string()
                };
                let escaped = html_escape(&truncated);
                format!(
                    "<span class=\"text-negative text-[11px]\" title=\"{escaped}\">{escaped}</span>"
                )
            })
            .unwrap_or_default();

        let extra_col = if show_priority {
            format!(
                "<td class=\"py-2 pr-3 text-[13px] text-ink-500 text-center\">{}</td>",
                task.priority
            )
        } else {
            format!(
                "<td class=\"py-2 pr-3 text-[13px] text-ink-500 mono\">{}</td>",
                html_escape(&task.updated_at)
            )
        };

        let _ = write!(
            rows,
            "<tr class=\"border-b border-line last:border-0\">\
               <td class=\"py-2 px-3 text-[13px] mono text-ink-700\">{}/{}</td>\
               <td class=\"py-2 px-3 text-[13px] mono\">{}</td>\
               <td class=\"py-2 px-3 text-[13px]\">{}</td>\
               <td class=\"py-2 px-3 text-[13px]\">{badge}</td>\
               <td class=\"py-2 px-3 text-[13px] text-ink-500\">{}/{}</td>\
               {extra_col}\
               <td class=\"py-2 px-3 text-[13px]\">{error_cell}</td>\
             </tr>",
            html_escape(&task.registry),
            html_escape(&task.repository),
            html_escape(&task.tag),
            html_escape(&task.task),
            task.attempts,
            task.max_attempts,
        );
    }

    let extra_header = if show_priority {
        "<th class=\"py-2 px-3 text-[11px] font-medium text-ink-500 uppercase tracking-wider\">Priority</th>"
    } else {
        "<th class=\"py-2 px-3 text-[11px] font-medium text-ink-500 uppercase tracking-wider\">Completed</th>"
    };

    format!(
        "<div class=\"overflow-x-auto border border-line rounded-lg\">\
           <table class=\"w-full text-left\">\
             <thead>\
               <tr class=\"border-b border-line bg-surfaceMuted\">\
                 <th class=\"py-2 px-3 text-[11px] font-medium text-ink-500 uppercase tracking-wider\">Package</th>\
                 <th class=\"py-2 px-3 text-[11px] font-medium text-ink-500 uppercase tracking-wider\">Tag</th>\
                 <th class=\"py-2 px-3 text-[11px] font-medium text-ink-500 uppercase tracking-wider\">Task</th>\
                 <th class=\"py-2 px-3 text-[11px] font-medium text-ink-500 uppercase tracking-wider\">Status</th>\
                 <th class=\"py-2 px-3 text-[11px] font-medium text-ink-500 uppercase tracking-wider\">Attempts</th>\
                 {extra_header}\
                 <th class=\"py-2 px-3 text-[11px] font-medium text-ink-500 uppercase tracking-wider\">Error</th>\
               </tr>\
             </thead>\
             <tbody>{rows}</tbody>\
           </table>\
         </div>"
    )
}

/// Return an HTML badge span for a task status string.
fn status_badge_html(status: &str) -> &'static str {
    match status {
        "pending" => {
            "<span class=\"px-2 py-0.5 rounded-full text-[11px] font-medium \
             bg-amber-100 text-amber-800\">pending</span>"
        }
        "in_progress" => {
            "<span class=\"px-2 py-0.5 rounded-full text-[11px] font-medium \
             bg-blue-100 text-blue-800\">in progress</span>"
        }
        "completed" => {
            "<span class=\"px-2 py-0.5 rounded-full text-[11px] font-medium \
             bg-emerald-100 text-emerald-800\">completed</span>"
        }
        "failed" => {
            "<span class=\"px-2 py-0.5 rounded-full text-[11px] font-medium \
             bg-red-100 text-red-800\">failed</span>"
        }
        _ => "",
    }
}

/// Render an error page when the API is unreachable.
fn render_error(message: &str) -> String {
    let body = Division::builder()
        .class("pt-8 max-w-lg")
        .heading_1(|h1| {
            h1.class(format!("{} mb-4", typography::H1_CLASS))
                .text("Status")
        })
        .paragraph(|p| {
            p.class(typography::BODY_CLASS)
                .text(format!("Could not load queue status: {message}"))
        })
        .build();
    layout::document_with_nav("Status", &body.to_string())
}

/// Minimal HTML entity escaping.
fn html_escape(s: &str) -> String {
    crate::escape::escape_html_text(s)
}
