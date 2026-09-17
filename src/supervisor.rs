use std::path::Path;

use anyhow::Result;

pub fn install(exe: &Path) -> Result<bool> {
    platform::install(exe)
}

pub fn uninstall() {
    platform::uninstall();
}

#[cfg(target_os = "macos")]
mod platform {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use anyhow::{Context, Result};

    use super::super::settings::log_path;

    const LABEL: &str = "com.eyesoff.proxy";

    fn plist_path() -> Result<PathBuf> {
        let home = std::env::home_dir().context("could not find your home directory")?;
        Ok(home.join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
    }

    fn escape(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    }

    pub(super) fn plist_contents(exe: &Path, log: &Path) -> String {
        let exe = escape(&exe.display().to_string());
        let log = escape(&log.display().to_string());
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n\
             <dict>\n\
             \t<key>Label</key><string>{LABEL}</string>\n\
             \t<key>ProgramArguments</key>\n\
             \t<array>\n\
             \t\t<string>{exe}</string>\n\
             \t\t<string>start</string>\n\
             \t</array>\n\
             \t<key>RunAtLoad</key><true/>\n\
             \t<key>KeepAlive</key><true/>\n\
             \t<key>ProcessType</key><string>Background</string>\n\
             \t<key>StandardOutPath</key><string>{log}</string>\n\
             \t<key>StandardErrorPath</key><string>{log}</string>\n\
             </dict>\n\
             </plist>\n"
        )
    }

    pub fn install(exe: &Path) -> Result<bool> {
        let path = plist_path()?;
        let contents = plist_contents(exe, &log_path());
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, &contents)?;
        let _ = Command::new("launchctl").arg("unload").arg("-w").arg(&path).output();
        let _ = Command::new("pkill").args(["-f", &format!("{} start", exe.display())]).output();
        Command::new("launchctl").arg("load").arg("-w").arg(&path).output().context("could not run launchctl")?;
        Ok(true)
    }

    pub fn uninstall() {
        if let Ok(path) = plist_path() {
            let _ = Command::new("launchctl").arg("unload").arg("-w").arg(&path).output();
            let _ = fs::remove_file(&path);
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use anyhow::{Context, Result};

    const UNIT: &str = "eyesoff.service";

    fn unit_path() -> Result<PathBuf> {
        let home = std::env::home_dir().context("could not find your home directory")?;
        Ok(home.join(".config/systemd/user").join(UNIT))
    }

    pub(super) fn unit_contents(exe: &Path) -> String {
        format!(
            "[Unit]\nDescription=eyesoff proxy\n\n\
             [Service]\nExecStart={} start\nRestart=always\nRestartSec=1\n\n\
             [Install]\nWantedBy=default.target\n",
            exe.display()
        )
    }

    fn systemctl(args: &[&str]) -> std::io::Result<std::process::Output> {
        Command::new("systemctl").arg("--user").args(args).output()
    }

    pub fn install(exe: &Path) -> Result<bool> {
        if !systemctl(&["--version"]).is_ok_and(|o| o.status.success()) {
            return Ok(false);
        }
        let path = unit_path()?;
        let contents = unit_contents(exe);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, &contents)?;
        systemctl(&["daemon-reload"]).context("could not run systemctl")?;
        let enabled = systemctl(&["enable", UNIT]).context("could not run systemctl")?;
        let restarted = systemctl(&["restart", UNIT]).context("could not run systemctl")?;
        Ok(enabled.status.success() && restarted.status.success())
    }

    pub fn uninstall() {
        let _ = systemctl(&["disable", "--now", UNIT]);
        if let Ok(path) = unit_path() {
            let _ = fs::remove_file(&path);
        }
        let _ = systemctl(&["daemon-reload"]);
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use anyhow::{Context, Result};

    use super::super::settings::log_path;

    const TASK_NAME: &str = "eyesoff-proxy";

    fn escape(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    }

    pub(super) fn task_xml(exe: &Path, log: &Path) -> String {
        let command = escape(&format!("\"{}\" start >> \"{}\" 2>&1", exe.display(), log.display()));
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <Task version=\"1.2\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\n\
             \t<Triggers><LogonTrigger><Enabled>true</Enabled></LogonTrigger></Triggers>\n\
             \t<Principals><Principal id=\"Author\"><LogonType>InteractiveToken</LogonType></Principal></Principals>\n\
             \t<Settings>\n\
             \t\t<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>\n\
             \t\t<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>\n\
             \t\t<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>\n\
             \t\t<RestartOnFailure><Interval>PT1M</Interval><Count>9999</Count></RestartOnFailure>\n\
             \t\t<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>\n\
             \t</Settings>\n\
             \t<Actions Context=\"Author\">\n\
             \t\t<Exec><Command>cmd.exe</Command><Arguments>/C {command}</Arguments></Exec>\n\
             \t</Actions>\n\
             </Task>\n"
        )
    }

    fn schtasks(args: &[&str]) -> std::io::Result<std::process::Output> {
        Command::new("schtasks").args(args).output()
    }

    pub fn install(exe: &Path) -> Result<bool> {
        let xml_path = std::env::temp_dir().join("eyesoff-task.xml");
        fs::write(&xml_path, task_xml(exe, &log_path()))?;
        let image = exe.file_name().unwrap().to_string_lossy();
        let _ = Command::new("taskkill").args(["/F", "/FI", &format!("IMAGENAME eq {image}")]).output();
        let created =
            schtasks(&["/Create", "/TN", TASK_NAME, "/XML", &xml_path.to_string_lossy(), "/F"]).context("could not run schtasks")?;
        let _ = fs::remove_file(&xml_path);
        if !created.status.success() {
            return Ok(false);
        }
        schtasks(&["/Run", "/TN", TASK_NAME]).context("could not run schtasks")?;
        Ok(true)
    }

    pub fn uninstall() {
        let _ = schtasks(&["/End", "/TN", TASK_NAME]);
        let _ = schtasks(&["/Delete", "/TN", TASK_NAME, "/F"]);
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
mod platform {
    use std::path::Path;

    use anyhow::Result;

    pub fn install(_exe: &Path) -> Result<bool> {
        Ok(false)
    }

    pub fn uninstall() {}
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use std::path::Path;

    #[test]
    fn plist_names_the_binary_and_restarts_it() {
        let plist = super::platform::plist_contents(Path::new("/Users/me/.local/bin/eyesoff"), Path::new("/tmp/eyesoff.log"));
        assert!(plist.contains("<string>/Users/me/.local/bin/eyesoff</string>"));
        assert!(plist.contains("<string>start</string>"));
        assert!(plist.contains("<key>KeepAlive</key><true/>"));
        assert!(plist.contains("<key>RunAtLoad</key><true/>"));
    }
}

#[cfg(all(test, target_os = "linux"))]
mod linux_tests {
    use std::path::Path;

    #[test]
    fn unit_names_the_binary_and_restarts_it() {
        let unit = super::platform::unit_contents(Path::new("/home/me/.local/bin/eyesoff"));
        assert!(unit.contains("ExecStart=/home/me/.local/bin/eyesoff start"));
        assert!(unit.contains("Restart=always"));
    }
}

#[cfg(all(test, target_os = "windows"))]
mod windows_tests {
    use std::path::Path;

    #[test]
    fn task_names_the_binary_and_restarts_it() {
        let xml = super::platform::task_xml(Path::new(r"C:\Users\me\eyesoff.exe"), Path::new(r"C:\Temp\eyesoff.log"));
        assert!(xml.contains(r"C:\Users\me\eyesoff.exe"));
        assert!(xml.contains("start &gt;&gt; "));
        assert!(xml.contains("<RestartOnFailure>"));
        assert!(xml.contains("<LogonTrigger>"));
    }
}
