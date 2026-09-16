use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

pub fn paste_from_clipboard(name: &str, env_file: Option<&Path>, command: &[String]) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new().context("could not open the clipboard")?;
    let value = clipboard.get_text().unwrap_or_default();
    let saved = paste(name, &value, env_file, command)?;
    clipboard.clear().context("saved, but could not clear the clipboard")?;
    println!("{saved}, clipboard cleared");
    Ok(())
}

pub fn paste(name: &str, value: &str, env_file: Option<&Path>, command: &[String]) -> Result<String> {
    let mut chars = name.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        bail!("{name:?} is not a valid variable name");
    }
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        bail!("the clipboard is empty or has whitespace in it, copy just the secret");
    }
    let length = value.chars().count();

    match (env_file, command) {
        (Some(path), []) => {
            write_env(path, name, value)?;
            Ok(format!("{name} saved to {} ({length} chars)", path.display()))
        }
        (None, [program, args @ ..]) => {
            let mut child =
                Command::new(program).args(args).stdin(Stdio::piped()).spawn().with_context(|| format!("could not run `{program}`"))?;
            let written = child.stdin.take().unwrap().write_all(value.as_bytes());
            let status = child.wait()?;
            if !status.success() || written.is_err() {
                bail!("`{program}` failed ({status}), clipboard left as is");
            }
            Ok(format!("{name} saved to {program} ({length} chars)"))
        }
        _ => bail!("pass either --env FILE or a command after --"),
    }
}

fn write_env(path: &Path, name: &str, value: &str) -> Result<()> {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let existing = match fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => return Err(e).with_context(|| format!("could not read {}", path.display())),
    };

    let prefix = format!("{name}=");
    let entry = format!("{name}={value}");
    let mut replaced = false;
    let mut lines: Vec<String> = existing
        .as_deref()
        .unwrap_or_default()
        .lines()
        .map(|line| {
            if line.starts_with(&prefix) {
                replaced = true;
                entry.clone()
            } else {
                line.to_string()
            }
        })
        .collect();
    if !replaced {
        lines.push(entry);
    }

    let file_name = path.file_name().context("not a file path")?.to_string_lossy();
    let temp = path.with_file_name(format!(".{file_name}.eyesoff"));
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mode = fs::metadata(&path).map(|m| m.permissions().mode() & 0o777).unwrap_or(0o600);
        options.mode(mode);
    }
    let mut file = options.open(&temp).with_context(|| format!("could not write {}", temp.display()))?;
    file.write_all((lines.join("\n") + "\n").as_bytes())?;
    file.sync_all()?;
    fs::rename(&temp, &path).with_context(|| format!("could not replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const FAKE: &str = concat!("sk_", "live_", "51HxQ7vK2mNp8RtL4wYz");

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("eyesoff-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn replaces_the_existing_line_and_keeps_the_rest() {
        let dir = scratch_dir("replace");
        let env = dir.join(".env");
        fs::write(&env, "DATABASE_URL=postgres://localhost/app\nSTRIPE_SECRET_KEY=old\n").unwrap();

        let message = paste("STRIPE_SECRET_KEY", &format!("{FAKE}\n"), Some(&env), &[]).unwrap();
        assert!(!message.contains(FAKE));
        assert_eq!(fs::read_to_string(&env).unwrap(), format!("DATABASE_URL=postgres://localhost/app\nSTRIPE_SECRET_KEY={FAKE}\n"));

        paste("OTHER_KEY", FAKE, Some(&env), &[]).unwrap();
        assert!(fs::read_to_string(&env).unwrap().ends_with(&format!("STRIPE_SECRET_KEY={FAKE}\nOTHER_KEY={FAKE}\n")));
    }

    #[cfg(unix)]
    #[test]
    fn keeps_file_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch_dir("modes");
        let existing = dir.join(".env");
        fs::write(&existing, "A=1\n").unwrap();
        fs::set_permissions(&existing, fs::Permissions::from_mode(0o640)).unwrap();
        paste("KEY", FAKE, Some(&existing), &[]).unwrap();
        assert_eq!(fs::metadata(&existing).unwrap().permissions().mode() & 0o777, 0o640);

        let created = dir.join("new.env");
        paste("KEY", FAKE, Some(&created), &[]).unwrap();
        assert_eq!(fs::metadata(&created).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn pipes_the_value_into_a_command() {
        let dir = scratch_dir("pipe");
        let out = dir.join("piped");
        let command = ["sh".to_string(), "-c".to_string(), format!("cat > '{}'", out.display())];
        paste("KEY", FAKE, None, &command).unwrap();
        assert_eq!(fs::read_to_string(&out).unwrap(), FAKE);

        let error = paste("KEY", FAKE, None, &["false".to_string()]).unwrap_err().to_string();
        assert!(error.contains("clipboard left as is") && !error.contains(FAKE));
    }

    #[test]
    fn rejects_bad_input_without_echoing_it() {
        let env = scratch_dir("bad").join(".env");
        assert!(paste("KEY", "two words", Some(&env), &[]).unwrap_err().to_string().contains("whitespace"));
        assert!(paste("KEY", "", Some(&env), &[]).is_err());
        assert!(paste("1KEY", FAKE, Some(&env), &[]).is_err());
        assert!(paste("KEY", FAKE, None, &[]).is_err());
        assert!(!env.exists());
    }
}
