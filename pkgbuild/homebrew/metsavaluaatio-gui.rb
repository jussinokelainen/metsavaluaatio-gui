class MetsavaluaatioGui < Formula
  desc "Metsäomaisuuden arviointi GPUI-työpöytäsovelluksella"
  homepage "ssh://git@gitlab.internal.nokehub.net:222/jussinokelainen/metsavaluaatio-gui.git"
  url "ssh://git@gitlab.internal.nokehub.net:222/jussinokelainen/metsavaluaatio-gui.git",
      using: :git,
      tag: "0.1.0"
  version "0.1.0"
  license "AGPL-3.0-only"

  depends_on "rust" => :build
  depends_on "cmake" => :build
  depends_on "sqlite"

  def install
    system "cargo", "install", "--bin", "metsaval-gui", *std_cargo_args
  end
end
