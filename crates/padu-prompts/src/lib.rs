//! Shared instructions and built-in resources, independent of agent transports.

pub mod resources;

use std::path::Path;

const WORKSPACE: &str = include_str!("../instructions/workspace.md");
const ARTIFACTS: &str = include_str!("../instructions/artifacts.md");

pub fn workspace() -> &'static str {
    WORKSPACE.trim()
}

pub fn artifacts() -> &'static str {
    ARTIFACTS.trim()
}

/// The catalog, less the resources named in `hidden` — those of surfaces
/// switched off.
pub fn resource_catalog(hidden: &[&str]) -> String {
    format!("Available Padu resources:\n{}", resources::catalog(hidden))
}

/// Instructions for callers with or without a bound project.
pub fn tool_context(bound: bool, hidden: &[&str]) -> String {
    let project = if bound {
        "Project tools work in the project bound to this connection unless a call names \
another one. A named project must be one Padu has open; the project argument takes its \
directory path."
    } else {
        "Project tools take the project's directory path. Resources are independent of projects."
    };
    format!(
        "{}\n\n{project}\n\n{}\n\n{}",
        workspace(),
        artifacts(),
        resource_catalog(hidden)
    )
}

/// Current session state alongside static instructions, refreshed each turn.
///
/// `session` is the session's entry number; without one the context names no
/// session.
pub fn session_context(
    cwd: &Path,
    session: Option<u64>,
    mcp_available: bool,
    hidden: &[&str],
) -> String {
    let mut context = format!(
        "Padu session context\n{}\n\nCurrent project: {}\n",
        workspace(),
        cwd.display(),
    );
    if let Some(number) = session {
        context.push_str(&format!("Current session: #{number}\n"));
    }
    context.push('\n');
    if mcp_available {
        context.push_str(artifacts());
        context.push_str("\n\n");
        context.push_str(&resource_catalog(hidden));
    } else {
        context.push_str(
            "Padu's MCP connection is unavailable. Required reference documents cannot be \
            loaded. Report this limitation before tasks that depend on Padu tools or reference \
            documents; do not guess their behavior.",
        );
    }
    context
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resources_list() {
        let list = resources::list();
        assert!(!list.is_empty());
        assert!(list.iter().any(|r| r.name == "browser"));
    }

    #[test]
    fn test_read_browser_resource() {
        let res = resources::read("browser").expect("browser resource exists");
        assert_eq!(res.name, "browser");
        assert_eq!(res.uri(), "padu://resources/browser");
        assert!(res.content.contains("# The in-app browser"));
    }

    #[test]
    fn test_catalog_hidden() {
        let cat = resources::catalog(&["browser"]);
        assert!(!cat.contains("browser"));
        let full = resources::catalog(&[]);
        assert!(full.contains("browser"));
    }

    #[test]
    fn test_tool_context() {
        let ctx = tool_context(true, &[]);
        assert!(ctx.contains("Padu is a native desktop workspace"));
        assert!(ctx.contains("Available Padu resources:"));
    }

    #[test]
    fn test_session_context() {
        let path = Path::new("/tmp/my-project");
        let ctx = session_context(path, Some(12), true, &[]);
        assert!(ctx.contains("/tmp/my-project"));
        assert!(ctx.contains("Current session: #12"));
    }
}
