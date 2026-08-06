class Rglint < Formula
  desc "GraphQL schema and operation linter"
  homepage "https://github.com/Intellicode/rglint"
  version "0.1.0"

  # Stub for a future Homebrew tap. Replace the version and SHA256 values for
  # each release before publishing the formula.
  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/Intellicode/rglint/releases/download/v#{version}/rglint-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_AARCH64_APPLE_DARWIN_SHA256"
    else
      url "https://github.com/Intellicode/rglint/releases/download/v#{version}/rglint-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_APPLE_DARWIN_SHA256"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/Intellicode/rglint/releases/download/v#{version}/rglint-#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_AARCH64_UNKNOWN_LINUX_GNU_SHA256"
    else
      url "https://github.com/Intellicode/rglint/releases/download/v#{version}/rglint-#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_X86_64_UNKNOWN_LINUX_GNU_SHA256"
    end
  end

  def install
    bin.install "rglint"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/rglint --version")
  end
end
