# Homebrew formula for CelestineMD (source build).
#
# Publish under your own tap, e.g.:
#   gh repo create nek0pi/homebrew-celestine --public
#   mkdir -p Formula && cp packaging/homebrew/celestine-md.rb Formula/
#   git commit && git push
#   brew tap nek0pi/celestine
#   brew install --HEAD celestine-md
#
# After the first GitHub release tag (v0.1.0), uncomment `url`/`sha256` and
# set sha256 from:
#   curl -sL https://github.com/nek0pi/celestine-md/archive/refs/tags/v0.1.0.tar.gz | shasum -a 256

class CelestineMd < Formula
  desc "Ultralight Obsidian-inspired Markdown notes (Rust / egui)"
  homepage "https://github.com/nek0pi/celestine-md"
  license "MIT"
  head "https://github.com/nek0pi/celestine-md.git", branch: "main"

  # Uncomment after v0.1.0 source tarball exists on GitHub:
  # url "https://github.com/nek0pi/celestine-md/archive/refs/tags/v0.1.0.tar.gz"
  # sha256 "REPLACE_WITH_SHA256_OF_SOURCE_TARBALL"
  # version "0.1.0"

  depends_on "rust" => :build

  on_linux do
    depends_on "pkgconf" => :build
  end

  def install
    system "cargo", "build", "--release", "-p", "celestine-app"
    bin.install "target/release/celestine"
    (pkgshare/"themes").install Dir["themes/*.css"]
    (pkgshare/"config").install Dir["config/keybindings.*.toml"]
  end

  def caveats
    <<~EOS
      Themes ship under #{pkgshare}/themes
      Optional: copy a keybinding profile to ~/.config/celestine/keybindings.toml
      Launch: celestine ~/Notes
    EOS
  end

  test do
    assert_predicate bin/"celestine", :exist?
  end
end
