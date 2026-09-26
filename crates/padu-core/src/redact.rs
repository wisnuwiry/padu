//! Secret scrubbing for logs and database text (P0-04).
//!
//! Padu never writes provider tokens itself, but automation carries remote
//! handles whose payloads can echo credentials (Git tokens in clone URLs,
//! bearer tokens in tool output). Scrub with [`Redactor`] before logging
//! or appending audit `detail`; the audit writer stores exactly what it
//! is given, so redaction at the call site is the guarantee.

/// Placeholder that replaces every secret occurrence.
pub const REDACTED: &str = "[redacted]";

/// Replaces known secret strings inside otherwise-human-readable text.
#[derive(Clone, Debug, Default)]
pub struct Redactor {
    /// Longest first, so overlapping secrets redact as one placeholder.
    secrets: Vec<String>,
}

impl Redactor {
    /// Build from any iterator of secret strings. Empty strings are
    /// dropped: replacing them would redact the gaps between characters.
    pub fn new(secrets: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut secrets: Vec<String> = secrets
            .into_iter()
            .map(Into::into)
            .filter(|secret| !secret.is_empty())
            .collect();
        secrets.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
        secrets.dedup();
        Self { secrets }
    }

    /// Return `text` with every secret occurrence replaced by [`REDACTED`].
    pub fn redact(&self, text: &str) -> String {
        let mut scrubbed = text.to_owned();
        for secret in &self.secrets {
            scrubbed = scrubbed.replace(secret, REDACTED);
        }
        scrubbed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_text_passes_through_untouched() {
        let redactor = Redactor::new(["ghp_example_secret_token_abc123"]);
        let text = "opened pull request #8 for task 7";
        assert_eq!(redactor.redact(text), text);
    }

    #[test]
    fn every_occurrence_of_every_secret_is_replaced() {
        let first = "ghp_example_secret_token_abc123";
        let second = "xoxb-other-secret-456";
        let redactor = Redactor::new([first, second]);

        let scrubbed = redactor.redact(&format!(
            "token {first} used, then {second}, then {first} again"
        ));
        assert!(!scrubbed.contains(first));
        assert!(!scrubbed.contains(second));
        assert_eq!(scrubbed.matches(REDACTED).count(), 3);
    }

    #[test]
    fn overlapping_secrets_redact_as_one() {
        let redactor = Redactor::new(["secret", "super-secret-token"]);
        assert_eq!(
            redactor.redact("key=super-secret-token"),
            format!("key={REDACTED}")
        );
    }

    #[test]
    fn empty_secrets_are_ignored() {
        let redactor = Redactor::new(["", "real-secret"]);
        assert_eq!(redactor.redact("nothing here"), "nothing here");
        assert_eq!(
            redactor.redact("has real-secret inside"),
            format!("has {REDACTED} inside")
        );
    }
}
