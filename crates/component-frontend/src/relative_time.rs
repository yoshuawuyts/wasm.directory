//! Human-friendly relative ages ("3 days ago", "2 weeks ago").

use chrono::{DateTime, Utc};

/// Describe how long ago `then` was, relative to `now`.
///
/// Uses days for the first two weeks and weeks for the first two months,
/// then months and years. Future timestamps (e.g. from clock skew) read as
/// "today".
#[must_use]
pub(crate) fn relative_age(then: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let days = (now - then).num_days();
    match days {
        ..=0 => "today".to_owned(),
        1..14 => plural(days, "day"),
        14..60 => plural(days / 7, "week"),
        60..365 => plural(days / 30, "month"),
        _ => plural(days / 365, "year"),
    }
}

fn plural(n: i64, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{n} {unit}s ago")
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use super::*;

    fn ago(delta: TimeDelta) -> String {
        let now = DateTime::parse_from_rfc3339("2026-09-24T12:00:00Z")
            .expect("valid timestamp")
            .with_timezone(&Utc);
        relative_age(now - delta, now)
    }

    #[test]
    fn picks_a_readable_unit() {
        assert_eq!(ago(TimeDelta::hours(-3)), "today");
        assert_eq!(ago(TimeDelta::hours(23)), "today");
        assert_eq!(ago(TimeDelta::days(1)), "1 day ago");
        assert_eq!(ago(TimeDelta::days(13)), "13 days ago");
        assert_eq!(ago(TimeDelta::days(14)), "2 weeks ago");
        assert_eq!(ago(TimeDelta::days(59)), "8 weeks ago");
        assert_eq!(ago(TimeDelta::days(60)), "2 months ago");
        assert_eq!(ago(TimeDelta::days(364)), "12 months ago");
        assert_eq!(ago(TimeDelta::days(365)), "1 year ago");
        assert_eq!(ago(TimeDelta::days(800)), "2 years ago");
    }
}
