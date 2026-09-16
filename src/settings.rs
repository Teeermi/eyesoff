use std::fs;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

pub const PORT: u16 = 8787;
const BASE_URL: &str = "http://127.0.0.1:8787";
const DENY_RULE: &str = "Bash(pbpaste *)";

pub fn add_to_claude_settings() -> Result<()> {
    let path = settings_path()?;
    let mut settings = match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).with_context(|| format!("could not parse {}", path.display()))?,
        Err(e) if e.kind() == ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e).with_context(|| format!("could not read {}", path.display())),
    };
    let changed = add_eyesoff(&mut settings).with_context(|| format!("left {} as is", path.display()))?;

    if !proxy_listening() {
        start_proxy()?;
        if !(0..20).any(|_| {
            sleep(Duration::from_millis(250));
            proxy_listening()
        }) {
            bail!("the proxy didn't start, so {} was left as is. See {}", path.display(), log_path().display());
        }
        println!("Started the eyesoff proxy on {BASE_URL}");
    }

    if changed {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;
        }
        fs::write(&path, serde_json::to_string_pretty(&settings)? + "\n").with_context(|| format!("could not write {}", path.display()))?;
        println!("Pointed Claude Code at eyesoff in {}", path.display());
    } else {
        println!("{} already points at eyesoff", path.display());
    }
    Ok(())
}

pub fn remove_from_claude_settings() -> Result<()> {
    let path = settings_path()?;
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

fn settings_path() -> Result<PathBuf> {
    let dir = match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => std::env::home_dir().context("could not find your home directory")?.join(".claude"),
    };
    Ok(dir.join("settings.json"))
}

fn proxy_listening() -> bool {
    TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, PORT).into(), Duration::from_secs(1)).is_ok()
}

fn log_path() -> PathBuf {
    std::env::temp_dir().join("eyesoff.log")
}

fn start_proxy() -> Result<()> {
    let log = fs::File::create(log_path()).with_context(|| format!("could not create {}", log_path().display()))?;
    let mut command = Command::new(std::env::current_exe()?);
    command.arg("start").stdin(Stdio::null()).stdout(log.try_clone()?).stderr(log);
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0000_0008 | 0x0000_0200);
    command.spawn().context("could not start the proxy")?;
    Ok(())
}

fn add_eyesoff(settings: &mut Value) -> Result<bool> {
    let root = settings.as_object_mut().context("it isn't a JSON object")?;

    let env = root.entry("env").or_insert_with(|| json!({})).as_object_mut().context("\"env\" isn't an object")?;
    let url_added = match env.get("ANTHROPIC_BASE_URL").and_then(Value::as_str) {
        Some(BASE_URL) => false,
        Some(url) => bail!("ANTHROPIC_BASE_URL already points at {url}"),
        None => {
            env.insert("ANTHROPIC_BASE_URL".into(), json!(BASE_URL));
            true
        }
    };

    let deny = root
        .entry("permissions")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .context("\"permissions\" isn't an object")?
        .entry("deny")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .context("\"permissions.deny\" isn't a list")?;
    let rule_added = !deny.iter().any(|rule| rule == DENY_RULE);
    if rule_added {
        deny.push(json!(DENY_RULE));
    }

    Ok(url_added || rule_added)
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
    fn adds_to_existing_settings_once() {
        let mut settings = json!({"model": "opus", "permissions": {"deny": ["Bash(rm *)"]}});
        assert!(add_eyesoff(&mut settings).unwrap());
        assert_eq!(
            serde_json::to_string(&settings).unwrap(),
            r#"{"model":"opus","permissions":{"deny":["Bash(rm *)","Bash(pbpaste *)"]},"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:8787"}}"#
        );
        assert!(!add_eyesoff(&mut settings).unwrap());

        assert!(remove_eyesoff(&mut settings));
        assert_eq!(settings, json!({"model": "opus", "permissions": {"deny": ["Bash(rm *)"]}}));

        let mut settings = json!({});
        assert!(add_eyesoff(&mut settings).unwrap());
        assert_eq!(settings, json!({"env": {"ANTHROPIC_BASE_URL": BASE_URL}, "permissions": {"deny": [DENY_RULE]}}));

        let mut settings = json!({"env": {"ANTHROPIC_BASE_URL": "https://gateway.example.com"}});
        assert!(add_eyesoff(&mut settings).is_err());
    }

    #[test]
    fn removes_only_what_setup_added() {
        let mut settings = json!({
            "env": {"ANTHROPIC_BASE_URL": "http://127.0.0.1:8787"},
            "permissions": {"allow": ["Bash(ls *)"], "deny": ["Bash(pbpaste *)", "Bash(rm *)"]},
            "model": "opus"
        });
        assert!(remove_eyesoff(&mut settings));
        assert_eq!(settings, json!({"permissions": {"allow": ["Bash(ls *)"], "deny": ["Bash(rm *)"]}, "model": "opus"}));
        assert_eq!(
            serde_json::to_string(&settings).unwrap(),
            r#"{"permissions":{"allow":["Bash(ls *)"],"deny":["Bash(rm *)"]},"model":"opus"}"#
        );

        let mut settings =
            json!({"env": {"ANTHROPIC_BASE_URL": "http://127.0.0.1:8787", "DEBUG": "1"}, "permissions": {"deny": ["Bash(pbpaste *)"]}});
        assert!(remove_eyesoff(&mut settings));
        assert_eq!(settings, json!({"env": {"DEBUG": "1"}}));

        let mut settings = json!({"env": {"ANTHROPIC_BASE_URL": "https://gateway.example.com"}});
        assert!(!remove_eyesoff(&mut settings));
        assert_eq!(settings, json!({"env": {"ANTHROPIC_BASE_URL": "https://gateway.example.com"}}));
    }
}
