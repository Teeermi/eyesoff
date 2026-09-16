---
name: eyesoff-setup
description: >-
  Use when the user wants to set up, install, enable, configure, uninstall or
  remove eyesoff, or asks to keep API keys/secrets out of what Claude Code
  sends to the model. Triggers on "set up eyesoff", "install eyesoff",
  "uninstall eyesoff", "hide secrets from Claude", "keep secrets out of
  context", "route Claude Code through eyesoff".
---

# Setting up eyesoff

Do every step yourself, in order. Never point Claude Code at the proxy before
it is confirmed running: with `ANTHROPIC_BASE_URL` set and nothing listening,
Claude Code can't reach the API at all.

1. Run `eyesoff --version`. If it isn't found, also try `~/.local/bin/eyesoff`
   and, on Windows, `$LOCALAPPDATA/Programs/eyesoff/eyesoff.exe`. If it's
   missing, install it:
   - macOS or Linux: `curl -fsSL https://raw.githubusercontent.com/Teeermi/eyesoff/main/install.sh | sh`
   - Windows: `powershell -NoProfile -Command "irm https://raw.githubusercontent.com/Teeermi/eyesoff/main/install.ps1 | iex"`

   If the install directory isn't on PATH in this session yet, use the full
   path to the binary for the next step.
2. Check whether the proxy is listening:
   `curl -s -o /dev/null --max-time 1 http://127.0.0.1:8787; echo $?`.
   Any exit code except 7 means it is. If it isn't, start it detached so it
   outlives this session:
   `nohup eyesoff start </dev/null >"${TMPDIR:-/tmp}/eyesoff.log" 2>&1 &`,
   wait a second and check again. Stop and report the log if it still isn't
   listening.
3. Merge this into `~/.claude/settings.json`, creating the file if needed.
   Keep every existing key and append to an existing `deny` list instead of
   replacing it:
   ```json
   {
     "env": { "ANTHROPIC_BASE_URL": "http://127.0.0.1:8787" },
     "permissions": { "deny": ["Bash(pbpaste *)"] }
   }
   ```
   Auto mode may deny this edit because it changes where Claude Code sends
   requests. If it does, don't retry and don't print the user's existing
   settings. Tell them the change needs their approval: switch out of auto mode
   with Shift+Tab and say "set up eyesoff" again, or add the two keys above
   themselves.
4. Tell the user eyesoff is set up. Claude Code picks up the new API address
   right away, including in sessions that are already open, so the proxy must
   stay running. Ask them to restart Claude Code once so the plugin can give
   Claude the rules for handling keys. From then on the plugin starts the proxy
   at the beginning of every session if it isn't running, so nothing needs to
   go into CLAUDE.md.

## Uninstalling

Don't run the uninstall yourself: this session goes through the proxy and
loses its connection as soon as the proxy stops. Give the user the command to
run in a terminal outside Claude Code:

- macOS or Linux: `curl -fsSL https://raw.githubusercontent.com/Teeermi/eyesoff/main/uninstall.sh | sh`
- Windows: `irm https://raw.githubusercontent.com/Teeermi/eyesoff/main/uninstall.ps1 | iex`

It removes eyesoff from the settings first, then the plugin, the proxy and the
binary.
