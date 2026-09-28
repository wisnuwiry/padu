//! Daemon-owned sidebar sort & grouping engine.
//!
//! The desktop (`apps/desktop/src/app/sidebar.rs`) and web
//! (`apps/web/src/lib/sidebar-presentation.ts`) previously each implemented
//! their own session sorting, date-bucket grouping, project grouping, and
//! recent-window pagination. This module is the single canonical
//! implementation: the daemon runs it over its authoritative task state and
//! serves the result through `Command::GetSidebarGroups`. Clients render the
//! returned groups and keep only presentation state (collapsed sections,
//! reveal counts are sent back as request input).

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::model::{AgentSession, Project};

/// Canonical recent-window for project grouping: sessions older than this are
/// paginated behind "show more". Unifies the previous desktop (3 days) and web
/// (7 days) divergence on the web value.
pub const SIDEBAR_PROJECT_RECENT_WINDOW_SECONDS: u64 = 7 * 24 * 60 * 60;

/// How many older sessions one "show more" reveals inside a project section,
/// on top of the always-visible minimum below.
pub const SIDEBAR_PROJECT_REVEAL_BATCH: u32 = 30;

/// Minimum older sessions always visible per project group, even when stale.
///
/// The default visible set is item-count-based combined with the date rule:
/// every recent session shows, plus at least this many older sessions, so a
/// quiet project still lists its latest work instead of rendering an empty
/// section. "Show more" reveals further older sessions beyond this minimum.
pub const SIDEBAR_PROJECT_MIN_VISIBLE_SESSIONS: usize = 5;

/// How the sidebar groups task history.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SidebarGrouping {
    #[default]
    Project,
    Updated,
}

/// Direction of task history inside the current grouping.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SidebarOrdering {
    #[default]
    Newest,
    Oldest,
}

/// Calendar bucket for `SidebarGrouping::Updated`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SidebarDateGroup {
    Today,
    Yesterday,
    Week,
    Month,
    Year,
    More,
}

impl SidebarDateGroup {
    pub const ALL: [Self; 6] = [
        Self::Today,
        Self::Yesterday,
        Self::Week,
        Self::Month,
        Self::Year,
        Self::More,
    ];

    pub fn index(self) -> usize {
        match self {
            Self::Today => 0,
            Self::Yesterday => 1,
            Self::Week => 2,
            Self::Month => 3,
            Self::Year => 4,
            Self::More => 5,
        }
    }
}

/// What a returned group contains.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SidebarGroupKind {
    Pinned,
    Updated,
    Project,
    Projectless,
}

/// One group in the sidebar view: the daemon owns ordering, membership, and
/// pagination; the client owns rendering and collapsed state.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SidebarGroupView {
    /// Stable id: `pinned`, `updated:<date-group>`, `project:<uuid>`,
    /// `projectless`.
    pub id: String,
    pub kind: SidebarGroupKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub date_group: Option<SidebarDateGroup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub project_id: Option<Uuid>,
    /// Visible session ids in display order. Older sessions beyond the
    /// revealed count are omitted when `has_more` is set.
    pub session_ids: Vec<Uuid>,
    pub has_more: bool,
}

/// Recency for sidebar ordering and date groups. A submitted turn promotes
/// the task immediately, while metadata edits such as a rename do not; a
/// task with no turns stays anchored to when it was created.
pub fn sidebar_session_timestamp(session: &AgentSession) -> u64 {
    session.last_reply_at.unwrap_or(session.created_at)
}

/// Sort sessions in place. Pinned sessions sort first (preserving
/// timestamp order within pinned/unpinned), matching the web engine; the
/// desktop engine extracted pinned afterwards with identical visible results.
///
/// Generic over `Borrow<AgentSession>` so the daemon (owned snapshots) and
/// the desktop (reference views over its synced snapshot) share one
/// implementation.
pub fn sort_sidebar_sessions<T>(sessions: &mut [T], ordering: SidebarOrdering)
where
    T: std::borrow::Borrow<AgentSession>,
{
    match ordering {
        SidebarOrdering::Newest => sessions.sort_by(|left, right| {
            let (left, right) = (left.borrow(), right.borrow());
            pinned_rank(right)
                .cmp(&pinned_rank(left))
                .then_with(|| {
                    sidebar_session_timestamp(right).cmp(&sidebar_session_timestamp(left))
                })
                .then_with(|| left.id.cmp(&right.id))
        }),
        SidebarOrdering::Oldest => sessions.sort_by(|left, right| {
            let (left, right) = (left.borrow(), right.borrow());
            pinned_rank(right)
                .cmp(&pinned_rank(left))
                .then_with(|| {
                    sidebar_session_timestamp(left).cmp(&sidebar_session_timestamp(right))
                })
                .then_with(|| left.id.cmp(&right.id))
        }),
    }
}

fn pinned_rank(session: &AgentSession) -> u8 {
    u8::from(session.pinned_at.is_some())
}

/// Convert a unix timestamp to a client-local calendar date using the
/// client-supplied UTC offset. The daemon may run in a different timezone
/// than the client, so the offset travels with the request.
pub fn session_local_date(timestamp: u64, local_utc_offset_secs: i32) -> Option<NaiveDate> {
    let shifted = i64::try_from(timestamp)
        .ok()?
        .checked_add(i64::from(local_utc_offset_secs))?;
    chrono::DateTime::<chrono::Utc>::from_timestamp(shifted, 0).map(|date| date.date_naive())
}

pub fn date_group_for_dates(session_date: NaiveDate, today: NaiveDate) -> SidebarDateGroup {
    if session_date >= today {
        return SidebarDateGroup::Today;
    }
    if today.pred_opt() == Some(session_date) {
        return SidebarDateGroup::Yesterday;
    }
    let week_start = today
        .checked_sub_days(chrono::Days::new(u64::from(
            today.weekday().num_days_from_monday(),
        )))
        .unwrap_or(today);
    if session_date >= week_start {
        return SidebarDateGroup::Week;
    }
    if session_date.year() == today.year() && session_date.month() == today.month() {
        return SidebarDateGroup::Month;
    }
    if session_date.year() == today.year() {
        return SidebarDateGroup::Year;
    }
    SidebarDateGroup::More
}

use chrono::Datelike as _;

/// Canonical sidebar view builder used by the daemon RPC handler.
///
/// - Filters to started, unarchived sessions.
/// - Sorts by [`sort_sidebar_sessions`].
/// - Pinned sessions form the leading group.
/// - `Updated` grouping buckets the rest by calendar date; `Oldest` ordering
///   reverses group order.
/// - `Project` grouping preserves first-seen (global sorted) project order
///   with projectless sessions trailing. Each group shows every recent
///   session plus at least [`SIDEBAR_PROJECT_MIN_VISIBLE_SESSIONS`] older
///   ones; further older sessions stay behind `revealed_older` counts keyed
///   by group id.
#[allow(clippy::too_many_arguments)]
pub fn build_sidebar_groups(
    projects: &[Project],
    sessions: &[AgentSession],
    grouping: SidebarGrouping,
    ordering: SidebarOrdering,
    today: NaiveDate,
    now_secs: u64,
    local_utc_offset_secs: i32,
    revealed_older: &HashMap<String, u32>,
) -> Vec<SidebarGroupView> {
    let projectless_ids: HashSet<Uuid> = projects
        .iter()
        .filter(|project| project.is_projectless())
        .map(|project| project.id)
        .collect();

    let mut started: Vec<&AgentSession> = sessions
        .iter()
        .filter(|session| session.archived_at.is_none() && session.has_started())
        .collect();
    sort_sidebar_sessions(&mut started, ordering);

    let pinned_ids: Vec<Uuid> = started
        .iter()
        .filter(|session| session.pinned_at.is_some())
        .map(|session| session.id)
        .collect();
    let unpinned: Vec<&AgentSession> = started
        .iter()
        .filter(|session| session.pinned_at.is_none())
        .copied()
        .collect();

    let mut groups = Vec::new();
    if !pinned_ids.is_empty() {
        groups.push(SidebarGroupView {
            id: sidebar_group_id(SidebarGroupKind::Pinned, None, None),
            kind: SidebarGroupKind::Pinned,
            date_group: None,
            project_id: None,
            session_ids: pinned_ids,
            has_more: false,
        });
    }

    match grouping {
        SidebarGrouping::Updated => {
            let mut buckets: [Vec<Uuid>; 6] = std::array::from_fn(|_| Vec::new());
            for session in unpinned {
                let timestamp = sidebar_session_timestamp(session);
                let date = session_local_date(timestamp, local_utc_offset_secs).unwrap_or(today);
                buckets[date_group_for_dates(date, today).index()].push(session.id);
            }
            let mut order = SidebarDateGroup::ALL;
            if ordering == SidebarOrdering::Oldest {
                order.reverse();
            }
            for date_group in order {
                let ids = std::mem::take(&mut buckets[date_group.index()]);
                if ids.is_empty() {
                    continue;
                }
                groups.push(SidebarGroupView {
                    id: sidebar_group_id(SidebarGroupKind::Updated, Some(date_group), None),
                    kind: SidebarGroupKind::Updated,
                    date_group: Some(date_group),
                    project_id: None,
                    session_ids: ids,
                    has_more: false,
                });
            }
        }
        SidebarGrouping::Project => {
            let recent_cutoff = now_secs.saturating_sub(SIDEBAR_PROJECT_RECENT_WINDOW_SECONDS);
            let timestamps: HashMap<Uuid, u64> = unpinned
                .iter()
                .map(|session| (session.id, sidebar_session_timestamp(session)))
                .collect();

            let mut ordered: Vec<(Option<Uuid>, Vec<Uuid>)> = Vec::new();
            let mut indexes: HashMap<Uuid, usize> = HashMap::new();
            let mut projectless: Vec<Uuid> = Vec::new();
            for session in unpinned {
                if projectless_ids.contains(&session.project_id) {
                    projectless.push(session.id);
                    continue;
                }
                let index = *indexes.entry(session.project_id).or_insert_with(|| {
                    let index = ordered.len();
                    ordered.push((Some(session.project_id), Vec::new()));
                    index
                });
                ordered[index].1.push(session.id);
            }
            if !projectless.is_empty() {
                ordered.push((None, projectless));
            }

            for (project_id, ids) in ordered {
                let (kind, id) = match project_id {
                    Some(project_id) => (
                        SidebarGroupKind::Project,
                        sidebar_group_id(SidebarGroupKind::Project, None, Some(project_id)),
                    ),
                    None => (
                        SidebarGroupKind::Projectless,
                        sidebar_group_id(SidebarGroupKind::Projectless, None, None),
                    ),
                };
                let revealed = revealed_older.get(&id).copied().unwrap_or(0) as usize;
                let (visible, has_more) =
                    paginate_project_sessions(&ids, &timestamps, recent_cutoff, revealed);
                groups.push(SidebarGroupView {
                    id,
                    kind,
                    date_group: None,
                    project_id,
                    session_ids: visible,
                    has_more,
                });
            }
        }
    }

    groups
}

fn paginate_project_sessions(
    sessions: &[Uuid],
    timestamps: &HashMap<Uuid, u64>,
    recent_cutoff: u64,
    revealed_older: usize,
) -> (Vec<Uuid>, bool) {
    let budget = revealed_older.saturating_add(SIDEBAR_PROJECT_MIN_VISIBLE_SESSIONS);
    let mut visible = Vec::with_capacity(sessions.len());
    let mut older_seen = 0usize;
    for session_id in sessions {
        let recent = timestamps
            .get(session_id)
            .is_some_and(|timestamp| *timestamp >= recent_cutoff);
        if recent || older_seen < budget {
            visible.push(*session_id);
        }
        if !recent {
            older_seen = older_seen.saturating_add(1);
        }
    }
    (visible, older_seen > budget)
}

fn date_group_name(group: SidebarDateGroup) -> &'static str {
    match group {
        SidebarDateGroup::Today => "today",
        SidebarDateGroup::Yesterday => "yesterday",
        SidebarDateGroup::Week => "week",
        SidebarDateGroup::Month => "month",
        SidebarDateGroup::Year => "year",
        SidebarDateGroup::More => "more",
    }
}

/// Stable wire id for a group, shared by the daemon response and the client's
/// `revealed_older` request map: `pinned`, `updated:<date-group>`,
/// `project:<uuid>`, `projectless`.
pub fn sidebar_group_id(
    kind: SidebarGroupKind,
    date_group: Option<SidebarDateGroup>,
    project_id: Option<Uuid>,
) -> String {
    match kind {
        SidebarGroupKind::Pinned => "pinned".to_owned(),
        SidebarGroupKind::Updated => format!(
            "updated:{}",
            date_group.map(date_group_name).unwrap_or("today")
        ),
        SidebarGroupKind::Project => match project_id {
            Some(project_id) => format!("project:{project_id}"),
            None => "projectless".to_owned(),
        },
        SidebarGroupKind::Projectless => "projectless".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ProviderKind;

    fn session_at(timestamp: u64) -> AgentSession {
        let mut session = AgentSession::new(Uuid::new_v4(), ProviderKind::Codex);
        session.created_at = timestamp;
        session.turns.push(crate::model::AgentTurn {
            id: Uuid::new_v4(),
            turn_count: 1,
            status: crate::model::TurnStatus::Completed,
            provider_turn_started: true,
            provider_resume_at: None,
            started_at: timestamp,
            completed_at: Some(timestamp),
            checkpoint: None,
        });
        session
    }

    #[test]
    fn groups_sessions_by_calendar_period() {
        let today = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let cases = [
            ((2026, 8, 12), SidebarDateGroup::Today),
            ((2026, 8, 11), SidebarDateGroup::Yesterday),
            ((2026, 8, 10), SidebarDateGroup::Week),
            ((2026, 8, 1), SidebarDateGroup::Month),
            ((2026, 1, 1), SidebarDateGroup::Year),
            ((2025, 12, 31), SidebarDateGroup::More),
        ];
        for ((year, month, day), expected) in cases {
            let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
            assert_eq!(date_group_for_dates(date, today), expected);
        }
    }

    #[test]
    fn recency_uses_last_reply_with_creation_fallback() {
        let mut renamed = session_at(10);
        renamed.last_reply_at = Some(20);
        renamed.updated_at = 1_000;
        let mut unanswered = AgentSession::new(Uuid::new_v4(), ProviderKind::Codex);
        unanswered.created_at = 30;

        assert_eq!(sidebar_session_timestamp(&renamed), 20);
        assert_eq!(sidebar_session_timestamp(&unanswered), 30);

        let mut sessions = vec![renamed.clone(), unanswered.clone()];
        sort_sidebar_sessions(&mut sessions, SidebarOrdering::Newest);
        assert_eq!(sessions[0].id, unanswered.id);
        sort_sidebar_sessions(&mut sessions, SidebarOrdering::Oldest);
        assert_eq!(sessions[0].id, renamed.id);
    }

    #[test]
    fn project_grouping_preserves_global_order_with_trailing_projectless() {
        let first_project = Uuid::from_u128(1);
        let second_project = Uuid::from_u128(2);
        let mut second = session_at(30);
        second.project_id = second_project;
        let mut first = session_at(20);
        first.project_id = first_project;
        let mut third = session_at(10);
        third.project_id = first_project;
        let projects = vec![
            Project {
                id: first_project,
                name: "First".into(),
                path: "/tmp/first".into(),
                created_at: 0,
                scripts: Vec::new(),
                linked_repo: None,
            },
            Project {
                id: second_project,
                name: "Second".into(),
                path: "/tmp/second".into(),
                created_at: 0,
                scripts: Vec::new(),
                linked_repo: None,
            },
        ];
        let today = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let groups = build_sidebar_groups(
            &projects,
            &[second.clone(), first.clone(), third.clone()],
            SidebarGrouping::Project,
            SidebarOrdering::Newest,
            today,
            50,
            0,
            &HashMap::new(),
        );
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].project_id, Some(second_project));
        assert_eq!(groups[0].session_ids, vec![second.id]);
        assert_eq!(groups[1].project_id, Some(first_project));
        assert_eq!(groups[1].session_ids, vec![first.id, third.id]);
    }

    #[test]
    fn project_pagination_reveals_older_history_in_batches() {
        let project_id = Uuid::from_u128(1);
        let projects = vec![Project {
            id: project_id,
            name: "P".into(),
            path: "/tmp/p".into(),
            created_at: 0,
            scripts: Vec::new(),
            linked_repo: None,
        }];
        let now = 1_000_000u64;
        let cutoff = now - SIDEBAR_PROJECT_RECENT_WINDOW_SECONDS;
        let sessions: Vec<AgentSession> = (0..36)
            .map(|index| {
                let mut session = session_at(cutoff - u64::try_from(index).unwrap_or(0));
                session.project_id = project_id;
                if index == 0 {
                    session.created_at = cutoff;
                    session.last_reply_at = Some(cutoff);
                    session.turns[0].started_at = cutoff;
                    session.turns[0].completed_at = Some(cutoff);
                }
                session
            })
            .collect();
        let today = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let groups = build_sidebar_groups(
            &projects,
            &sessions,
            SidebarGrouping::Project,
            SidebarOrdering::Newest,
            today,
            now,
            0,
            &HashMap::new(),
        );
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0].session_ids.len(),
            1 + SIDEBAR_PROJECT_MIN_VISIBLE_SESSIONS,
            "one recent session plus the always-visible older minimum",
        );
        assert!(groups[0].has_more);

        let mut revealed = HashMap::new();
        revealed.insert(groups[0].id.clone(), SIDEBAR_PROJECT_REVEAL_BATCH * 2);
        let groups = build_sidebar_groups(
            &projects,
            &sessions,
            SidebarGrouping::Project,
            SidebarOrdering::Newest,
            today,
            now,
            0,
            &revealed,
        );
        assert_eq!(groups[0].session_ids.len(), 36);
        assert!(!groups[0].has_more);
    }

    #[test]
    fn project_groups_keep_a_minimum_of_items_combined_with_the_date_rule() {
        let project_id = Uuid::from_u128(1);
        let projects = vec![Project {
            id: project_id,
            name: "P".into(),
            path: "/tmp/p".into(),
            created_at: 0,
            scripts: Vec::new(),
            linked_repo: None,
        }];
        let now = 1_000_000u64;
        let cutoff = now - SIDEBAR_PROJECT_RECENT_WINDOW_SECONDS;
        let stale = |index: u64| {
            let mut session = session_at(cutoff - 1 - index);
            session.project_id = project_id;
            session
        };
        let today = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let view = |sessions: &[AgentSession]| {
            build_sidebar_groups(
                &projects,
                sessions,
                SidebarGrouping::Project,
                SidebarOrdering::Newest,
                today,
                now,
                0,
                &HashMap::new(),
            )
            .pop()
            .expect("one project group")
        };

        // All stale: the minimum keeps the section from rendering empty.
        let sessions: Vec<AgentSession> = (0..10).map(stale).collect();
        let group = view(&sessions);
        assert_eq!(
            group.session_ids.len(),
            SIDEBAR_PROJECT_MIN_VISIBLE_SESSIONS
        );
        assert_eq!(
            group.session_ids,
            sessions[..SIDEBAR_PROJECT_MIN_VISIBLE_SESSIONS]
                .iter()
                .map(|session| session.id)
                .collect::<Vec<_>>(),
            "the newest older sessions stay visible",
        );
        assert!(group.has_more);

        // Fewer stale sessions than the minimum: everything shows, no pager.
        let sessions: Vec<AgentSession> = (0..3).map(stale).collect();
        let group = view(&sessions);
        assert_eq!(group.session_ids.len(), 3);
        assert!(!group.has_more);

        // Recent sessions add on top of the minimum via the date rule.
        let mut sessions: Vec<AgentSession> = (0..10).map(stale).collect();
        for index in 0..2u64 {
            let mut recent = session_at(cutoff + index);
            recent.project_id = project_id;
            sessions.push(recent);
        }
        let group = view(&sessions);
        assert_eq!(
            group.session_ids.len(),
            2 + SIDEBAR_PROJECT_MIN_VISIBLE_SESSIONS
        );
        assert!(group.has_more);
    }
}
