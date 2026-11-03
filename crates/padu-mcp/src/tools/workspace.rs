//! What the Padu window is showing.

use crate::{
    rail,
    tool::{Answer, Args, Outcome, Tool},
    tools::fields,
};
use serde_json::{Value, json};

pub static TOOLS: [Tool; 1] = [Tool {
    name: "workspace_focus",
    description: "The panels and entries the Padu window is showing now: one entry, \
        or every pane of an open space with the focused one marked.",
    schema: |_| fields(false, &[]),
    writes: false,
    deletes: false,
    call: focus,
}];

fn focus(_: Args<'_>) -> Outcome {
    let shown: Vec<Value> = rail::shown()
        .iter()
        .map(|entry| {
            json!({
                "project": entry.project,
                "kind": entry.kind,
                "id": entry.id,
                "focused": entry.focused,
            })
        })
        .collect();
    let text = match on_screen() {
        Some(text) => text,
        None => "the Padu window is showing no active panel".to_owned(),
    };
    Ok(Answer::said(text).with(json!({ "shown": shown })))
}

pub fn on_screen() -> Option<String> {
    let shown = rail::shown();
    if shown.is_empty() {
        return None;
    }
    let lines = shown
        .iter()
        .map(|entry| {
            format!(
                "- [{}] {} in {}{}",
                entry.kind,
                entry.id,
                entry.project.display(),
                if entry.focused && shown.len() > 1 {
                    " (focused)"
                } else {
                    ""
                },
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(lines)
}
