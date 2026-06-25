//! Secret-redaction utilities for logs and bug reports.
//!
//! Drop writes auth tokens into `drop.log` through several code paths (API
//! error bodies, handshake debug traces, Bearer-header echoes). Users submit
//! that log as part of bug reports. We run everything that leaves the local
//! machine — and the log itself, where we can intercept it — through this
//! redactor first.
//!
//! The function is intentionally over-eager: false positives here just mean a
//! base64-looking opaque blob shows up as `<REDACTED>` in a bug report, which
//! is fine. False negatives leak credentials, which isn't.

use std::sync::LazyLock;

use regex::Regex;

/// Patterns we replace in-line. Order matters — more specific patterns go
/// first so their replacements land before the generic token sweep.
static PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        // Authorization: Bearer <token>
        (
            Regex::new(r"(?i)(authorization\s*:\s*bearer\s+)([A-Za-z0-9._\-+/=]+)").unwrap(),
            "$1<REDACTED>",
        ),
        // Authorization: Basic <b64>
        (
            Regex::new(r"(?i)(authorization\s*:\s*basic\s+)([A-Za-z0-9+/=]+)").unwrap(),
            "$1<REDACTED>",
        ),
        // token=...&, token: ..., token="..."
        (
            Regex::new(
                r#"(?i)(token|access_token|refresh_token|api_key|handshake|secret|password|client_secret|clientId|client_id)\s*[=:]\s*['"]?([A-Za-z0-9._\-+/=]{12,})['"]?"#,
            )
            .unwrap(),
            "$1=<REDACTED>",
        ),
        // Cookie headers
        (
            Regex::new(r"(?i)(cookie\s*:\s*)([^\r\n]+)").unwrap(),
            "$1<REDACTED>",
        ),
        (
            Regex::new(r"(?i)(set-cookie\s*:\s*)([^\r\n]+)").unwrap(),
            "$1<REDACTED>",
        ),
    ]
});

/// Replace secret-looking substrings in `input`. Idempotent and safe to run
/// twice (redacting `<REDACTED>` is a no-op).
pub fn redact(input: &str) -> String {
    let mut out = input.to_string();
    for (re, replacement) in PATTERNS.iter() {
        out = re.replace_all(&out, *replacement).into_owned();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_bearer_token() {
        let s = "Authorization: Bearer abcdef.1234567890-xyz";
        assert_eq!(redact(s), "Authorization: Bearer <REDACTED>");
    }

    #[test]
    fn strips_token_query_param() {
        let s = "GET /api/v1/games?token=abc123def456ghi789 HTTP/1.1";
        let out = redact(s);
        assert!(out.contains("<REDACTED>"), "got: {out}");
        assert!(!out.contains("abc123def456ghi789"), "got: {out}");
    }

    #[test]
    fn strips_cookie_header() {
        let s = "Cookie: session=abcdef; csrf=1234";
        let out = redact(s);
        assert!(out.contains("<REDACTED>"), "got: {out}");
        assert!(!out.contains("abcdef"), "got: {out}");
    }

    #[test]
    fn leaves_ordinary_text_alone() {
        let s = "Launching umu-run with PROTONPATH=GE-Proton";
        assert_eq!(redact(s), s);
    }

    #[test]
    fn idempotent() {
        let s = "Authorization: Bearer tok_aaaaaaaaaaaaaaa";
        let once = redact(s);
        assert_eq!(redact(&once), once);
    }
}
