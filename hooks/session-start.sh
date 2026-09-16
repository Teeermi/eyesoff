#!/bin/sh
url=http://127.0.0.1:8787
PATH="$PATH:$HOME/.local/bin"

if [ "${ANTHROPIC_BASE_URL:-}" != "$url" ]; then
  cat <<'EOF'
{"systemMessage":"eyesoff is installed but not set up, so secrets are not filtered yet. Ask Claude to set up eyesoff.","hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"The eyesoff plugin is installed, but ANTHROPIC_BASE_URL does not point at the eyesoff proxy, so secrets in this session are NOT filtered before they reach the model. When the user asks to set up eyesoff, or before you handle API keys or other secrets, use the eyesoff-setup skill."}}
EOF
  exit 0
fi

up() {
  curl -s -o /dev/null --max-time 1 "$url"
  [ $? -ne 7 ]
}

if ! up && command -v eyesoff >/dev/null 2>&1; then
  nohup eyesoff start </dev/null >"${TMPDIR:-/tmp}/eyesoff.log" 2>&1 &
  sleep 1
fi

if ! up; then
  cat <<'EOF'
{"systemMessage":"eyesoff is not running on 127.0.0.1:8787 and could not be started, so Claude Code cannot reach the API. Run `eyesoff start` in another terminal."}
EOF
  exit 0
fi

cat <<'EOF'
{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"eyesoff is filtering this session: secret-looking strings in text and screenshots are removed before they reach the model. Rules for handling keys: never print, read or echo a secret value, and never read the clipboard (pbpaste, xclip, xsel, wl-paste, Get-Clipboard). To store a key shown in a web dashboard, click its Copy button, then run `eyesoff paste NAME --env .env`. To pass a key to a command that reads stdin, run `eyesoff paste NAME -- <command>`, for example `eyesoff paste API_TOKEN -- wrangler secret put API_TOKEN`. Text shown as [hidden by eyesoff] is a hidden secret; work around it and don't try to recover it. Drive the browser from this session (Claude in Chrome), never from the Chrome side panel, which talks to Anthropic directly."}}
EOF
