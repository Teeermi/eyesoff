---
name: eyesoff-setup
description: >-
  Use when the user wants to set up, install, enable or configure eyesoff, or
  asks to keep API keys/secrets out of what Claude Code sends to the model.
  Triggers on "set up eyesoff", "install eyesoff", "hide secrets from Claude",
  "keep secrets out of context", "route Claude Code through eyesoff".
---

# Setting up eyesoff

eyesoff only works if `ANTHROPIC_BASE_URL` is set to `http://127.0.0.1:8787`
**before** Claude Code starts - a plugin cannot change that once a session is
running. So this skill prepares the config and daemon, then tells the user to
restart the session; it cannot activate eyesoff for the session that invoked it.

1. Check the binary exists: `eyesoff --version`. If missing, tell the user to
   run `cargo install --git https://github.com/Teeermi/eyesoff` and stop here.
2. Edit `~/.claude/settings.json` (create it if missing) to add:
   ```json
   {
     "env": { "ANTHROPIC_BASE_URL": "http://127.0.0.1:8787" },
     "permissions": { "deny": ["Bash(pbpaste *)"] }
   }
   ```
   Merge into any existing keys, don't overwrite them.
3. Check whether eyesoff is already running: try `curl -s -o /dev/null --max-time 1 http://127.0.0.1:8787`.
   If it's not, start it detached so it outlives this session, e.g.
   `nohup eyesoff start >/tmp/eyesoff.log 2>&1 &`, and tell the user it's
   running and how to stop it (`pkill eyesoff`) or run it themselves in its
   own terminal instead.
4. Tell the user plainly: the change only takes effect on the **next** Claude
   Code session - this one is still talking to the API directly. Suggest they
   restart now.

Don't add CLAUDE.md instructions about handling keys unless the user asks -
that's a preference, not a requirement for eyesoff to work.
