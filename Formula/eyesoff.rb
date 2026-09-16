class Eyesoff < Formula
  desc "Local proxy that hides secrets from Claude Code before they reach the model"
  homepage "https://github.com/Teeermi/eyesoff"
  url "https://github.com/Teeermi/eyesoff.git",
      revision: "6db1ab59d0aa4751b8817f6cca6920cff8400cd9"
  version "0.1.0"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "eyesoff", shell_output("#{bin}/eyesoff --version")
  end
end
