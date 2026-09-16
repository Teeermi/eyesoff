# eyesoff

Let your coding agent set up API keys without ever seeing them.

Setting up env vars is boring, so you ask Claude Code to do it: open the Stripe dashboard, create a key, put it in `.env`. It can do that. The catch is that the key shows up in a screenshot, or in page text, or in `cat .env`, and all of that gets sent to the model.

eyesoff is a small local proxy between Claude Code and the Anthropic API. It strips secrets out of everything that leaves your machine. Text goes through pattern matching. Screenshots go through the OCR built into macOS, and anything that looks like a key gets a black box drawn over it. The key itself travels through your clipboard and never becomes part of the conversation.

![Claude Code sends text and screenshots to eyesoff on 127.0.0.1:8787, which strips secrets before forwarding to api.anthropic.com. Responses pass back untouched.](assets/diagram.svg)

## What a run looks like

1. You: "create a restricted Stripe key and add it to .env as STRIPE_SECRET_KEY".
2. The agent opens the dashboard and creates the key. The screenshot it takes has the key covered before it leaves your Mac.
3. The agent clicks Copy. The key is now in your clipboard, and the model has no view of that.
4. The agent runs `eyesoff paste STRIPE_SECRET_KEY --env .env`. eyesoff writes the clipboard into the file, clears the clipboard and prints `STRIPE_SECRET_KEY saved to .env (107 chars), clipboard cleared`. That line is all the model gets back.
5. If the agent reads `.env` later, it sees `STRIPE_SECRET_KEY=[hidden by eyesoff]`.

## Install

eyesoff is a single binary written in Rust. Until prebuilt releases are out, build it with cargo:

```sh
cargo install --git https://github.com/Teeermi/eyesoff
```

## Set up Claude Code

Start the proxy and leave it running:

```sh
eyesoff start
```

Point Claude Code at it in `~/.claude/settings.json`:

```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "http://127.0.0.1:8787"
  },
  "permissions": {
    "deny": ["Bash(pbpaste *)"]
  }
}
```

When eyesoff isn't running, Claude Code can't reach the API at all. That's on purpose: nothing goes out unchecked. Works with a claude.ai subscription login.

Then tell the agent how to handle keys, for example in `~/.claude/CLAUDE.md`:

```
Never print or read secret values, and never run pbpaste.
To store a key from a web dashboard, click its Copy button and run
`eyesoff paste NAME --env .env`. To send it to a command that reads stdin,
run `eyesoff paste NAME -- <command>`, e.g. `eyesoff paste API_TOKEN -- wrangler secret put API_TOKEN`.
```

Drive the browser from Claude Code (the Claude in Chrome integration), not from the Chrome side panel. The side panel talks to Anthropic on its own and never goes through eyesoff.

## Commands

`eyesoff start [--port 8787]` runs the proxy. Each request gets one log line, with a note when something was hidden:

```
POST /v1/messages?beta=true 200  hid 2 strings, 1 screenshot
```

`eyesoff paste NAME --env FILE` puts the clipboard into `FILE` as `NAME=value`. An existing `NAME=` line is replaced, anything else is left alone. New files are created with mode 600.

`eyesoff paste NAME -- command [args]` pipes the clipboard into the command's stdin. If the command fails, the clipboard is kept so you can try again.

## Limits

Be clear about what this is. It keeps secrets from leaking by accident. It is not a sandbox.

- Detection is a heuristic. It catches the known prefixes in [`prefixes.txt`](prefixes.txt) and strings of 20+ characters with digits and mixed case. A short token with no known prefix will get through.
- It also hides things that aren't secrets, like some hashes and base64. The agent sees `[hidden by eyesoff]` and usually works around it.
- OCR can miss text that is tiny, rotated or broken up in odd ways.
- An agent that really wants a key can still get it out, for example by encoding it first. eyesoff won't stop that.
- If a screenshot can't be checked, it's replaced with a note instead of being sent.
- Clipboard managers keep history. Exclude your browser in yours, or delete the entry.
- Claude Code's local transcripts in `~/.claude` still contain the original screenshots and output. Only what goes to the API is cleaned.
- Screenshots are only checked on macOS for now. On Linux and Windows the proxy still runs, but every screenshot is removed instead of checked.

## Contributing

Missing a provider? Adding one is a single line in [`prefixes.txt`](prefixes.txt). See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
