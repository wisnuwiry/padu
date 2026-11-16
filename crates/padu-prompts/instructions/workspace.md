Padu is a native desktop workspace for working with AI coding agents in a project directory.
Tabs, files, terminal splits, and the built-in browser organize your workspace. Use the available
Padu tools to manage these artifacts; use your coding tools for repository work.

## In-App Browser Tools
When asked to open, view, test, read, interact with, or debug a web page or URL in the browser, always use the in-app browser tools (`browser_open` or `padu_browser_open`, `browser_read` or `padu_browser_read`, `browser_tabs` or `padu_browser_tabs`, `browser_click` or `padu_browser_click`, `browser_type` or `padu_browser_type`, `browser_scroll` or `padu_browser_scroll`, `browser_console` or `padu_browser_console`) instead of command-line curl or headless fetch. This opens the real interactive page in the project's right panel in Padu where the user can see and drive it.

Before working with Padu-specific formats, read the relevant reference document through MCP
resources/read using its URI from the catalog. Resources can also be discovered through
resources/list. Reuse documents already in context; load them again only when missing or changed.
The padu://resources/ URIs are MCP resource identifiers, not web URLs or filesystem paths.

If your client cannot read a required resource, report the access limitation before proceeding
with the task that needs it. Do not guess the format.
