//! Presentation helpers specific to the Notes workspace.

pub(super) fn format_note_time_ago(seconds: u64) -> String {
    match seconds {
        0..=59 => tr!("sidebar.just_now"),
        60..=3_599 => tr!("sidebar.minutes_ago", count = seconds / 60),
        3_600..=86_399 => tr!("sidebar.hours_ago", count = seconds / 3_600),
        _ => tr!("sidebar.days_ago", count = seconds / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::format_note_time_ago;

    #[test]
    fn formats_note_age_boundaries() {
        assert_eq!(format_note_time_ago(30), "just now");
        assert_eq!(format_note_time_ago(90), "1m");
        assert_eq!(format_note_time_ago(3_600), "1h");
        assert_eq!(format_note_time_ago(86_400), "1d");
    }
}
