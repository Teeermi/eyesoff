import base64
import os
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

import eyesoff

HERE = Path(__file__).resolve().parent
FAKE_STRIPE = "sk_" + "live_" + "51Hx" + "Q7vK2mNp8RtL4wYz" * 4
FAKE_TOKEN = "i7r1BcTs" + "z8eOrG40lxLF1fdL" + "fgVp04VVy9VExL7C"


def hide(text):
    stats = {"strings": 0, "screenshots": 0}
    return eyesoff.redact_text(text, stats), stats["strings"]


def test_text():
    for secret in [FAKE_STRIPE, FAKE_TOKEN, "ghp_" + "a1B2c3D4e5F6g7H8i9J0", "AKIA" + "IOSFODNN7EXAMPLE1"]:
        redacted, count = hide(f"STRIPE_SECRET_KEY={secret}\n")
        assert secret not in redacted and count == 1, secret
        assert redacted.startswith("STRIPE_SECRET_KEY="), redacted

    for harmless in [
        "8a461400-2f98-41a5-8e47-00f596ad2124",
        "/private/tmp/claude-501/-Users-termi-repo-app/scratchpad/proxy.py",
        "commit 9216f2f7e84a65f20bd929aa01bc3d4e5f6a7b8c",
        "mcp__claude_ai_Google_Calendar__complete_authentication",
        "toolu_01XyZ9aB8cD7eF6gH5iJ4kL3",
        "pk_live_...9f3a",
        "The quick brown fox jumps over the lazy dog 42 times.",
    ]:
        assert hide(harmless) == (harmless, 0), harmless


def test_scrub_leaves_structure_alone():
    body = {
        "model": "claude-sonnet-5",
        "system": [{"type": "text", "text": f"env has {FAKE_TOKEN}"}],
        "messages": [
            {"role": "assistant", "content": [{"type": "tool_use", "id": "toolu_01XyZ9aB8cD7eF6gH5iJ4kL3", "name": "Bash", "input": {"command": "cat .env"}}]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_01XyZ9aB8cD7eF6gH5iJ4kL3", "content": f"KEY={FAKE_STRIPE}"}]},
        ],
    }
    stats = {"strings": 0, "screenshots": 0}
    out = eyesoff.scrub(body, stats)
    assert stats["strings"] == 2
    assert out["messages"][0] == body["messages"][0]
    assert out["messages"][1]["content"][0]["tool_use_id"] == "toolu_01XyZ9aB8cD7eF6gH5iJ4kL3"
    assert out["messages"][1]["content"][0]["content"] == f"KEY={eyesoff.HIDDEN}"
    assert FAKE_TOKEN not in out["system"][0]["text"]


def image_block(path_or_bytes):
    raw = path_or_bytes if isinstance(path_or_bytes, bytes) else (HERE / "testdata" / path_or_bytes).read_bytes()
    return {"type": "image", "source": {"type": "base64", "media_type": "image/jpeg", "data": base64.b64encode(raw).decode()}}


def test_screenshots():
    stats = {"strings": 0, "screenshots": 0}
    clean = image_block("clean.jpg")
    assert eyesoff.scrub(clean, stats) == clean and stats["screenshots"] == 0

    covered = eyesoff.scrub(image_block("dashboard.jpg"), stats)
    assert stats["screenshots"] == 1 and covered["source"]["media_type"] == "image/png"
    second_pass = subprocess.run([eyesoff.OCR], input=base64.b64decode(covered["source"]["data"]), capture_output=True)
    assert second_pass.returncode == 0 and second_pass.stdout == b"", "text still readable after covering"

    broken = eyesoff.scrub(image_block(b"not an image"), stats)
    assert broken == {"type": "text", "text": eyesoff.SCREENSHOT_REMOVED}


def run_cli(tmp, clipboard, *args):
    bin_dir = Path(tmp) / "bin"
    bin_dir.mkdir(exist_ok=True)
    (Path(tmp) / "clipboard").write_text(clipboard)
    (bin_dir / "pbpaste").write_text(f"#!/bin/sh\ncat '{tmp}/clipboard'\n")
    (bin_dir / "pbcopy").write_text(f"#!/bin/sh\ncat > '{tmp}/clipboard'\n")
    for tool in bin_dir.iterdir():
        tool.chmod(0o755)
    env = {**os.environ, "PATH": f"{bin_dir}:{os.environ['PATH']}"}
    return subprocess.run([sys.executable, HERE / "eyesoff.py", "paste", *args], capture_output=True, text=True, cwd=tmp, env=env)


def test_paste():
    with tempfile.TemporaryDirectory() as tmp:
        env_file = Path(tmp) / ".env"
        env_file.write_text("DATABASE_URL=postgres://localhost/app\nSTRIPE_SECRET_KEY=old\n")
        env_file.chmod(0o640)

        result = run_cli(tmp, FAKE_STRIPE + "\n", "STRIPE_SECRET_KEY", "--env", ".env")
        assert result.returncode == 0, result.stderr
        assert FAKE_STRIPE not in result.stdout + result.stderr
        assert env_file.read_text() == f"DATABASE_URL=postgres://localhost/app\nSTRIPE_SECRET_KEY={FAKE_STRIPE}\n"
        assert stat.S_IMODE(env_file.stat().st_mode) == 0o640
        assert (Path(tmp) / "clipboard").read_text() == ""

        result = run_cli(tmp, FAKE_TOKEN, "CF_API_TOKEN", "--env", "new.env")
        assert result.returncode == 0 and stat.S_IMODE((Path(tmp) / "new.env").stat().st_mode) == 0o600

        result = run_cli(tmp, FAKE_TOKEN, "CF_API_TOKEN", "--", "sh", "-c", "cat > piped")
        assert result.returncode == 0, result.stderr
        assert (Path(tmp) / "piped").read_text() == FAKE_TOKEN

        result = run_cli(tmp, FAKE_TOKEN, "CF_API_TOKEN", "--", "false")
        assert result.returncode != 0 and (Path(tmp) / "clipboard").read_text() == FAKE_TOKEN

        result = run_cli(tmp, "two words", "CF_API_TOKEN", "--env", ".env")
        assert result.returncode != 0 and "two" not in result.stderr


if __name__ == "__main__":
    for name, test in list(globals().items()):
        if name.startswith("test_"):
            test()
            print(f"ok  {name}")
