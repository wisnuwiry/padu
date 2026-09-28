//! Presentation helpers specific to the Notes workspace.

pub(super) fn format_note_time_ago(seconds: u64) -> String {
    match seconds {
        0..=59 => tr!("sidebar.just_now"),
        60..=3_599 => tr!("sidebar.minutes_ago", count = seconds / 60),
        3_600..=86_399 => tr!("sidebar.hours_ago", count = seconds / 3_600),
        _ => tr!("sidebar.days_ago", count = seconds / 86_400),
    }
}

pub(super) fn format_note_meta(project_name: &str, time_ago: &str, tags: &[String]) -> String {
    let mut meta = format!("{project_name} · {time_ago}");
    if !tags.is_empty() {
        const SHOWN_TAGS: usize = 3;
        let shown: Vec<String> = tags
            .iter()
            .take(SHOWN_TAGS)
            .map(|tag| format!("#{tag}"))
            .collect();
        meta.push_str(" · ");
        meta.push_str(&shown.join(" "));
        if tags.len() > SHOWN_TAGS {
            meta.push_str(&format!(" +{}", tags.len() - SHOWN_TAGS));
        }
    }
    meta
}

pub(super) fn next_layout_on_create(
    current: super::notes::NotesLayout,
) -> super::notes::NotesLayout {
    match current {
        super::notes::NotesLayout::Preview => super::notes::NotesLayout::Edit,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::{format_note_meta, format_note_time_ago, next_layout_on_create};
    use crate::app::notes::NotesLayout;

    #[test]
    fn formats_note_age_boundaries() {
        assert_eq!(format_note_time_ago(30), "just now");
        assert_eq!(format_note_time_ago(90), "1m");
        assert_eq!(format_note_time_ago(3_600), "1h");
        assert_eq!(format_note_time_ago(86_400), "1d");
    }

    #[test]
    fn formats_note_meta_with_and_without_tags() {
        assert_eq!(format_note_meta("Padu", "just now", &[]), "Padu · just now");
        assert_eq!(
            format_note_meta("Padu", "1h", &[String::from("design"), String::from("v1")]),
            "Padu · 1h · #design #v1"
        );
        let tags: Vec<String> = vec![
            "tag1".into(),
            "tag2".into(),
            "tag3".into(),
            "tag4".into(),
            "tag5".into(),
        ];
        assert_eq!(
            format_note_meta("Padu", "2d", &tags),
            "Padu · 2d · #tag1 #tag2 #tag3 +2"
        );
    }

    #[test]
    fn next_layout_on_create_changes_preview_to_edit_only() {
        assert_eq!(
            next_layout_on_create(NotesLayout::Preview),
            NotesLayout::Edit
        );
        assert_eq!(
            next_layout_on_create(NotesLayout::Split),
            NotesLayout::Split
        );
        assert_eq!(next_layout_on_create(NotesLayout::Edit), NotesLayout::Edit);
    }
}
