use std::collections::HashMap;
use std::hash::{BuildHasher, RandomState};
use std::sync::{LazyLock, Mutex};

use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use regex::{Captures, Regex};
use serde_json::{Value, json};

use crate::ocr;

pub const HIDDEN: &str = "[hidden by eyesoff]";
pub const SCREENSHOT_REMOVED: &str = "[screenshot removed by eyesoff: it could not be checked for secrets]";

const PREFIXES: &[&str] = &[
    "sk_live_", "sk_test_", "rk_live_", "rk_test_", "whsec_", "ghp_", "gho_", "ghu_", "ghs_",
    "github_pat_", "xoxb-", "xoxp-", "AKIA", "sk-ant-", "sk-proj-", "glpat-", "npm_",
];
const SAFE_PREFIXES: &[&str] = &["toolu_", "srvtoolu_", "msg_", "req_"];
const TEXT_KEYS: &[&str] = &["text", "content", "system"];

static TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z0-9_\-]{16,}").unwrap());
static CHECKED_IMAGES: LazyLock<Mutex<HashMap<u64, Option<String>>>> = LazyLock::new(Default::default);
static IMAGE_KEYS: LazyLock<RandomState> = LazyLock::new(RandomState::new);

#[derive(Default, Debug, PartialEq)]
pub struct Stats {
    pub strings: usize,
    pub screenshots: usize,
}

impl Stats {
    pub fn summary(&self) -> Option<String> {
        let parts: Vec<String> = [(self.strings, "string"), (self.screenshots, "screenshot")]
            .into_iter()
            .filter(|(n, _)| *n > 0)
            .map(|(n, what)| format!("{n} {what}{}", if n == 1 { "" } else { "s" }))
            .collect();
        (!parts.is_empty()).then(|| parts.join(", "))
    }
}

pub fn looks_like_secret(word: &str) -> bool {
    if word.contains("://") || word.contains("...") || SAFE_PREFIXES.iter().any(|p| word.starts_with(p)) {
        return false;
    }
    let length = word.chars().count();
    if let Some(rest) = PREFIXES.iter().find_map(|p| word.strip_prefix(p)) {
        return length >= 16 && rest.chars().any(|c| c.is_numeric());
    }
    length >= 20
        && word.chars().filter(|c| c.is_numeric()).count() >= 3
        && word.chars().any(char::is_uppercase)
        && word.chars().any(char::is_lowercase)
}

pub fn redact_text(text: &str, stats: &mut Stats) -> String {
    TOKEN
        .replace_all(text, |caps: &Captures| {
            if looks_like_secret(&caps[0]) {
                stats.strings += 1;
                HIDDEN.to_string()
            } else {
                caps[0].to_string()
            }
        })
        .into_owned()
}

pub fn scrub_body(raw: &[u8]) -> Result<(Vec<u8>, Stats)> {
    let mut body: Value = serde_json::from_slice(raw)?;
    let mut stats = Stats::default();
    scrub(&mut body, None, &mut stats);
    Ok((serde_json::to_vec(&body)?, stats))
}

pub fn scrub(value: &mut Value, key: Option<&str>, stats: &mut Stats) {
    if let Some(replacement) = checked_image(value, stats) {
        *value = replacement;
        return;
    }
    match value {
        Value::Object(map) => map.iter_mut().for_each(|(k, v)| scrub(v, Some(k), stats)),
        Value::Array(items) => items.iter_mut().for_each(|v| scrub(v, key, stats)),
        Value::String(text) if key.is_some_and(|k| TEXT_KEYS.contains(&k)) => *text = redact_text(text, stats),
        _ => {}
    }
}

fn checked_image(value: &Value, stats: &mut Stats) -> Option<Value> {
    if value["type"] != "image" || value["source"]["type"] != "base64" {
        return None;
    }
    let data = value["source"]["data"].as_str()?;
    match cover_image(data) {
        Ok(None) => None,
        Ok(Some(png)) => {
            stats.screenshots += 1;
            let mut block = value.clone();
            block["source"] = json!({"type": "base64", "media_type": "image/png", "data": png});
            Some(block)
        }
        Err(error) => {
            eprintln!("eyesoff: {error:#}");
            stats.screenshots += 1;
            Some(json!({"type": "text", "text": SCREENSHOT_REMOVED}))
        }
    }
}

fn cover_image(data: &str) -> Result<Option<String>> {
    let key = IMAGE_KEYS.hash_one(data);
    if let Some(known) = CHECKED_IMAGES.lock().unwrap().get(&key) {
        return Ok(known.clone());
    }
    let covered = ocr::cover_secrets(&BASE64.decode(data)?)?.map(|png| BASE64.encode(png));
    CHECKED_IMAGES.lock().unwrap().insert(key, covered.clone());
    Ok(covered)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const FAKE_STRIPE: &str = concat!("sk_", "live_", "51HxQ7vK2mNp8RtL4wYzQ7vK2mNp8RtL4wYzQ7vK2mNp8RtL4wYz");
    pub const FAKE_TOKEN: &str = concat!("i7r1BcTs", "z8eOrG40lxLF1fdL", "fgVp04VVy9VExL7C");

    fn hide(text: &str) -> (String, usize) {
        let mut stats = Stats::default();
        let out = redact_text(text, &mut stats);
        (out, stats.strings)
    }

    #[test]
    fn hides_secrets_in_text() {
        for secret in [FAKE_STRIPE, FAKE_TOKEN, concat!("ghp_", "a1B2c3D4e5F6g7H8i9J0"), concat!("AKIA", "IOSFODNN7EXAMPLE1")] {
            let (out, count) = hide(&format!("STRIPE_SECRET_KEY={secret}\n"));
            assert_eq!(out, format!("STRIPE_SECRET_KEY={HIDDEN}\n"), "{secret}");
            assert_eq!(count, 1);
        }
    }

    #[test]
    fn leaves_ordinary_text_alone() {
        for text in [
            "8a461400-2f98-41a5-8e47-00f596ad2124",
            "/private/tmp/claude-501/-Users-termi-repo-app/scratchpad/proxy.py",
            "commit 9216f2f7e84a65f20bd929aa01bc3d4e5f6a7b8c",
            "mcp__claude_ai_Google_Calendar__complete_authentication",
            "toolu_01XyZ9aB8cD7eF6gH5iJ4kL3",
            "pk_live_...9f3a",
            "set npm_config_registry or YARN_NPM_REGISTRY_SERVER",
            "The quick brown fox jumps over the lazy dog 42 times.",
        ] {
            assert_eq!(hide(text), (text.to_string(), 0), "{text}");
        }
    }

    #[test]
    fn scrub_only_touches_text() {
        let mut body = json!({
            "model": "claude-sonnet-5",
            "system": [{"type": "text", "text": format!("env has {FAKE_TOKEN}")}],
            "messages": [
                {"role": "assistant", "content": [{"type": "tool_use", "id": "toolu_01XyZ9aB8cD7eF6gH5iJ4kL3", "name": "Bash", "input": {"command": "cat .env"}}]},
                {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_01XyZ9aB8cD7eF6gH5iJ4kL3", "content": format!("KEY={FAKE_STRIPE}")}]}
            ]
        });
        let original = body.clone();
        let mut stats = Stats::default();
        scrub(&mut body, None, &mut stats);
        assert_eq!(stats.strings, 2);
        assert_eq!(body["messages"][0], original["messages"][0]);
        assert_eq!(body["messages"][1]["content"][0]["tool_use_id"], "toolu_01XyZ9aB8cD7eF6gH5iJ4kL3");
        assert_eq!(body["messages"][1]["content"][0]["content"], format!("KEY={HIDDEN}"));
        assert_eq!(body["system"][0]["text"], format!("env has {HIDDEN}"));
    }

    fn image_block(raw: &[u8]) -> Value {
        json!({"type": "image", "source": {"type": "base64", "media_type": "image/jpeg", "data": BASE64.encode(raw)}})
    }

    #[test]
    fn removes_screenshots_that_cannot_be_checked() {
        let mut block = image_block(b"not an image");
        let mut stats = Stats::default();
        scrub(&mut block, None, &mut stats);
        assert_eq!(block, json!({"type": "text", "text": SCREENSHOT_REMOVED}));
        assert_eq!(stats.screenshots, 1);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn covers_keys_in_screenshots() {
        let mut stats = Stats::default();
        let clean = image_block(include_bytes!("../testdata/clean.jpg"));
        let mut block = clean.clone();
        scrub(&mut block, None, &mut stats);
        assert_eq!(block, clean);
        assert_eq!(stats.screenshots, 0);

        let mut block = image_block(include_bytes!("../testdata/dashboard.jpg"));
        scrub(&mut block, None, &mut stats);
        assert_eq!(stats.screenshots, 1);
        assert_eq!(block["source"]["media_type"], "image/png");
        let covered = BASE64.decode(block["source"]["data"].as_str().unwrap()).unwrap();
        assert!(ocr::cover_secrets(&covered).unwrap().is_none(), "text still readable after covering");
    }
}
