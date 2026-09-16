#!/bin/sh
set -eu

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) target=aarch64-apple-darwin ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  Linux-x86_64 | Linux-amd64) target=x86_64-unknown-linux-musl ;;
  Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-musl ;;
  *)
    echo "eyesoff: no prebuilt binary for $(uname -sm). Build it with: cargo install --git https://github.com/Teeermi/eyesoff" >&2
    exit 1
    ;;
esac

dir="${EYESOFF_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$dir"
curl -fsSL "https://github.com/Teeermi/eyesoff/releases/latest/download/eyesoff-$target" -o "$dir/eyesoff.tmp"
chmod +x "$dir/eyesoff.tmp"
mv "$dir/eyesoff.tmp" "$dir/eyesoff"

echo "eyesoff installed to $dir/eyesoff"
shell="${SHELL:-sh}"
case "${shell##*/}" in
  zsh) rc="$HOME/.zshrc" ;;
  bash) rc="$HOME/.bashrc" ;;
  *) rc="$HOME/.profile" ;;
esac
case ":$PATH:" in
  *":$dir:"*) ;;
  *) echo "$dir is not on your PATH. Add it with: echo 'export PATH=\"$dir:\$PATH\"' >> $rc" ;;
esac
