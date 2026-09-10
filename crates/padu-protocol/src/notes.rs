use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// The compact representation used when listing a project's notes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub preview: String,
    pub revision: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

/// An immutable note snapshot embedded in a message or composer draft.
///
/// This deliberately contains only the content needed to preserve what the
/// provider saw; later edits to the source note must not change it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedNote {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub revision: u64,
}

/// The complete, editable representation of a note.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub content: String,
    pub revision: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CreateNote {
    pub project_id: Uuid,
    pub title: String,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNote {
    pub project_id: Uuid,
    pub note_id: Uuid,
    pub title: String,
    pub content: String,
    pub expected_revision: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_wire_shapes_round_trip() {
        let note = Note {
            id: Uuid::from_u128(1),
            project_id: Uuid::from_u128(2),
            title: "Plan".into(),
            content: "Details".into(),
            revision: 3,
            created_at: 4,
            updated_at: 5,
        };
        let json = serde_json::to_value(&note).unwrap();
        assert_eq!(json["projectId"], Uuid::from_u128(2).to_string());
        assert_eq!(json["revision"], 3);
        assert_eq!(serde_json::from_value::<Note>(json).unwrap(), note);
    }
}
