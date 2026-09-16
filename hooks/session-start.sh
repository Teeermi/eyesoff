#!/bin/sh
set -eu

if [ "${ANTHROPIC_BASE_URL:-}" != "http://127.0.0.1:8787" ]; then
  printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"eyesoff plugin: ANTHROPIC_BASE_URL is not set to http://127.0.0.1:8787, so secrets are NOT being filtered from what this session sends to the model. Use the eyesoff-setup skill to fix this."}}'
  exit 0
fi

if command -v curl >/dev/null 2>&1 && ! curl -s -o /dev/null --max-time 1 "$ANTHROPIC_BASE_URL"; then
  printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"eyesoff plugin: ANTHROPIC_BASE_URL points at eyesoff but nothing is listening on 127.0.0.1:8787. Run `eyesoff start` before doing anything with secrets this session."}}'
fi
