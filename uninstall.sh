#!/bin/sh
set -u

dir="${EYESOFF_INSTALL_DIR:-$HOME/.local/bin}"
bin="$(command -v eyesoff 2>/dev/null || true)"
[ -z "$bin" ] && [ -x "$dir/eyesoff" ] && bin="$dir/eyesoff"

if [ -n "$bin" ]; then
  if ! "$bin" uninstall; then
    echo "Couldn't update Claude Code settings, so nothing else was removed. Remove ANTHROPIC_BASE_URL from ~/.claude/settings.json and run this again." >&2
    exit 1
  fi
else
  echo "eyesoff isn't installed. If Claude Code can't connect, remove ANTHROPIC_BASE_URL from ~/.claude/settings.json."
fi

if command -v claude >/dev/null 2>&1; then
  claude plugin uninstall eyesoff@eyesoff >/dev/null 2>&1 && echo "Removed the eyesoff plugin from Claude Code"
  claude plugin marketplace remove eyesoff >/dev/null 2>&1
fi

pkill -f "eyesoff start" 2>/dev/null && echo "Stopped the eyesoff proxy"

if [ -f "$dir/eyesoff" ]; then
  rm -f "$dir/eyesoff" && echo "Deleted $dir/eyesoff"
fi
if command -v dpkg >/dev/null 2>&1 && dpkg -s eyesoff >/dev/null 2>&1; then
  sudo apt-get remove -y eyesoff
fi

echo "eyesoff is uninstalled. Restart any Claude Code sessions that are still open."
