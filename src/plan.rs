//! The Claude subscription plan, such as `Max 5x`.
//!
//! The session JSON doesn't carry the plan, so it's read from the account
//! profile Claude Code keeps in `.claude.json`. That file isn't a documented
//! interface: if its keys move, the plan is just left out.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Only the fields the plan needs; serde skips the rest of the file without
/// building it, about four times faster than parsing it into a `Value`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    oauth_account: Option<Account>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Account {
    organization_type: Option<String>,
    organization_rate_limit_tier: Option<String>,
}

/// `.claude.json` in `CLAUDE_CONFIG_DIR` when it's set, otherwise in `home`.
pub fn config_path(config_dir: Option<OsString>, home: &str) -> Option<PathBuf> {
    match config_dir.filter(|dir| !dir.is_empty()) {
        Some(dir) => Some(PathBuf::from(dir).join(".claude.json")),
        None if !home.is_empty() => Some(Path::new(home).join(".claude.json")),
        None => None,
    }
}

/// The plan from the config file at `path`, or None when the file can't be
/// read or holds no plan (signed in with an API key, for example).
pub fn read(path: &Path) -> Option<String> {
    parse(&std::fs::read(path).ok()?)
}

fn parse(raw: &[u8]) -> Option<String> {
    let account = serde_json::from_slice::<Config>(raw).ok()?.oauth_account?;
    label(
        account.organization_type.as_deref()?,
        account.organization_rate_limit_tier.as_deref(),
    )
}

/// `claude_pro` -> `Pro`; `claude_max` with tier `default_claude_max_5x` ->
/// `Max 5x`. Types are mapped generically so a new plan still gets a name.
fn label(kind: &str, tier: Option<&str>) -> Option<String> {
    let words: Vec<String> = kind
        .strip_prefix("claude_")?
        .split('_')
        .filter(|word| !word.is_empty())
        .map(capitalize)
        .collect();
    if words.is_empty() {
        return None;
    }
    let mut label = words.join(" ");

    // The usage multiplier ends the tier, such as `_5x`
    let multiplier = tier
        .and_then(|tier| tier.rsplit('_').next())
        .filter(|last| {
            last.strip_suffix('x')
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        });
    if let Some(multiplier) = multiplier {
        label.push(' ');
        label.push_str(multiplier);
    }
    Some(label)
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_types_get_readable_names() {
        assert_eq!(
            label("claude_pro", Some("default_claude_ai")).as_deref(),
            Some("Pro")
        );
        assert_eq!(label("claude_team", None).as_deref(), Some("Team"));
        assert_eq!(
            label("claude_enterprise", None).as_deref(),
            Some("Enterprise")
        );
        assert_eq!(label("claude_new_plan", None).as_deref(), Some("New Plan"));
    }

    #[test]
    fn max_shows_its_multiplier() {
        let max = |tier| label("claude_max", tier);
        assert_eq!(
            max(Some("default_claude_max_5x")).as_deref(),
            Some("Max 5x")
        );
        assert_eq!(
            max(Some("default_claude_max_20x")).as_deref(),
            Some("Max 20x")
        );
        assert_eq!(max(Some("default_claude_max")).as_deref(), Some("Max"));
        assert_eq!(max(Some("x")).as_deref(), Some("Max"));
        assert_eq!(max(None).as_deref(), Some("Max"));
    }

    #[test]
    fn unknown_types_show_nothing() {
        assert_eq!(label("api", None), None);
        assert_eq!(label("claude_", None), None);
        assert_eq!(label("", None), None);
    }

    #[test]
    fn parses_only_the_account_fields() {
        let raw = br#"{
            "numStartups": 12,
            "projects": {"/w": {"allowedTools": [], "history": [{"display": "hi"}]}},
            "oauthAccount": {
                "emailAddress": "someone@example.com",
                "organizationType": "claude_max",
                "organizationRateLimitTier": "default_claude_max_5x",
                "seatTier": null
            }
        }"#;
        assert_eq!(parse(raw).as_deref(), Some("Max 5x"));
    }

    #[test]
    fn missing_or_broken_config_shows_nothing() {
        assert_eq!(parse(br#"{"numStartups": 1}"#), None);
        assert_eq!(parse(br#"{"oauthAccount": null}"#), None);
        assert_eq!(
            parse(br#"{"oauthAccount": {"organizationType": null}}"#),
            None
        );
        assert_eq!(parse(br#"{"oauthAccount": {"organizationType": 7}}"#), None);
        assert_eq!(parse(b"not json"), None);
        assert_eq!(parse(b""), None);
    }

    #[test]
    fn config_dir_wins_over_home() {
        let home = if cfg!(windows) {
            r"C:\Users\alex"
        } else {
            "/home/alex"
        };
        let dir = if cfg!(windows) { r"D:\cfg" } else { "/cfg" };
        assert_eq!(
            config_path(Some(dir.into()), home),
            Some(Path::new(dir).join(".claude.json"))
        );
        assert_eq!(
            config_path(Some("".into()), home),
            Some(Path::new(home).join(".claude.json"))
        );
        assert_eq!(
            config_path(None, home),
            Some(Path::new(home).join(".claude.json"))
        );
        assert_eq!(config_path(None, ""), None);
    }

    #[test]
    fn missing_file_shows_nothing() {
        assert_eq!(read(Path::new("/definitely/not/here/.claude.json")), None);
    }
}
