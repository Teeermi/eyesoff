use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::Value;

const DENY_RULE: &str = "Bash(pbpaste *)";

pub fn remove_from_claude_settings() -> Result<()> {
    let dir = match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => std::env::home_dir().context("could not find your home directory")?.join(".claude"),
    };
    let path = dir.join("settings.json");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            println!("{} doesn't exist, nothing to remove", path.display());
            return Ok(());
        }
        Err(e) => return Err(e).with_context(|| format!("could not read {}", path.display())),
    };
    let mut settings: Value = serde_json::from_str(&text).with_context(|| format!("could not parse {}", path.display()))?;
    if remove_eyesoff(&mut settings) {
        fs::write(&path, serde_json::to_string_pretty(&settings)? + "\n").with_context(|| format!("could not write {}", path.display()))?;
        println!("Removed eyesoff from {}", path.display());
    } else {
        println!("{} doesn't point at eyesoff, left it as is", path.display());
    }
    Ok(())
}

fn remove_eyesoff(settings: &mut Value) -> bool {
    let Some(root) = settings.as_object_mut() else { return false };
    let mut changed = false;

    if let Some(env) = root.get_mut("env").and_then(Value::as_object_mut) {
        if env.get("ANTHROPIC_BASE_URL").and_then(Value::as_str).is_some_and(|url| url.starts_with("http://127.0.0.1:")) {
            env.shift_remove("ANTHROPIC_BASE_URL");
            changed = true;
        }
        if changed && env.is_empty() {
            root.shift_remove("env");
        }
    }

    if let Some(permissions) = root.get_mut("permissions").and_then(Value::as_object_mut)
        && let Some(deny) = permissions.get_mut("deny").and_then(Value::as_array_mut)
    {
        let before = deny.len();
        deny.retain(|rule| rule != DENY_RULE);
        if deny.len() != before {
            changed = true;
            if deny.is_empty() {
                permissions.shift_remove("deny");
            }
            if permissions.is_empty() {
                root.shift_remove("permissions");
            }
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn removes_only_what_setup_added() {
        let mut settings = json!({
            "env": {"ANTHROPIC_BASE_URL": "http://127.0.0.1:8787"},
            "permissions": {"allow": ["Bash(ls *)"], "deny": ["Bash(pbpaste *)", "Bash(rm *)"]},
            "model": "opus"
        });
        assert!(remove_eyesoff(&mut settings));
        assert_eq!(settings, json!({"permissions": {"allow": ["Bash(ls *)"], "deny": ["Bash(rm *)"]}, "model": "opus"}));
        assert_eq!(serde_json::to_string(&settings).unwrap(), r#"{"permissions":{"allow":["Bash(ls *)"],"deny":["Bash(rm *)"]},"model":"opus"}"#);

        let mut settings = json!({"env": {"ANTHROPIC_BASE_URL": "http://127.0.0.1:8787", "DEBUG": "1"}, "permissions": {"deny": ["Bash(pbpaste *)"]}});
        assert!(remove_eyesoff(&mut settings));
        assert_eq!(settings, json!({"env": {"DEBUG": "1"}}));

        let mut settings = json!({"env": {"ANTHROPIC_BASE_URL": "https://gateway.example.com"}});
        assert!(!remove_eyesoff(&mut settings));
        assert_eq!(settings, json!({"env": {"ANTHROPIC_BASE_URL": "https://gateway.example.com"}}));
    }
}
