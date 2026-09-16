# Contributing

## Adding a token format

Most API keys start with a fixed prefix, like `sk_live_` for Stripe or `ghp_` for GitHub. eyesoff keeps these in [`prefixes.txt`](prefixes.txt), one per line: the prefix, some spaces, and what it is.

```
ghp_              GitHub personal access token
```

To add one:

1. Add a line to `prefixes.txt`, next to the other entries for the same provider.
2. Run `cargo test`. Every line gets checked: the prefix can only use letters, digits, `_` and `-`, it can't be listed twice, and a fake token that starts with it has to get hidden.
3. Open a PR and link to something that shows the format, like the provider's docs, changelog or a blog post.

Never paste a real key into an issue or a PR, not even a revoked one.

A few things to know before you add a prefix:

- It only counts when at least 16 more characters follow it and at least one of them is a digit. That's what keeps names like `npm_config_registry` from getting hidden.
- Long random strings are hidden even without a prefix, so a format without a fixed prefix usually doesn't need an entry. If you have one that slips through anyway, open an issue.
- Prefixes with a `.` in them, like SendGrid's `SG.`, can't be matched yet.

## Reporting a false positive

If eyesoff hides something that isn't a secret and it gets in your agent's way, open an issue. You'll see `hid 1 string` in the proxy log when it happens. Include the text that got hidden if it's safe to share.

## Working on the code

```sh
cargo test
cargo clippy --all-targets
```

The screenshot tests only run on macOS, since that's the only platform with OCR so far.
