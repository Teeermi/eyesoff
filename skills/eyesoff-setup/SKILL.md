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

Do every step yourself, in order.

1. Run `eyesoff --version`. If it isn't found, also try `~/.local/bin/eyesoff`
   and, on Windows, `$LOCALAPPDATA/Programs/eyesoff/eyesoff.exe`.
2. If it's missing, install it. The installer also starts the proxy and points
   Claude Code at it in `~/.claude/settings.json`:
   - macOS or Linux: `curl -fsSL https://raw.githubusercontent.com/Teeermi/eyesoff/main/install.sh | sh`
   - Windows: `powershell -NoProfile -Command "irm https://raw.githubusercontent.com/Teeermi/eyesoff/main/install.ps1 | iex"`

   If it's already installed, run `eyesoff setup` instead, with the full path
   to the binary if it isn't on PATH.

   Auto mode may deny this because it changes where Claude Code sends
   requests. If it does, don't retry and don't edit the settings yourself. Ask
   the user to run the same command by typing it after `!`, for example
   `! eyesoff setup`.
3. If setup fails, report its output and stop. It leaves the settings alone
   when the proxy won't start or `ANTHROPIC_BASE_URL` already points somewhere
   else.
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
