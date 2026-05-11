cask "portsage" do
  version "0.8.0"
  sha256 "a6d9b17b354d7b8de42a2d995107c710b1851e9af2764fafc0acf9cc3c5eab61"

  url "https://github.com/essedev/portsage/releases/download/v#{version}/Portsage_#{version}_aarch64.dmg"
  name "Portsage"
  desc "Port allocation manager for macOS - ports under control"
  homepage "https://github.com/essedev/portsage"

  app "Portsage.app"

  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-cr", "#{appdir}/Portsage.app"]
  end
end
