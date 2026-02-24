cask "opsclaw-tools" do
  version "4.1.21"
  sha256 :no_check

  name "OpsClaw Tools"
  desc "Professional Account Management for AI Services"
  homepage "https://github.com/lbjlaq/OpsClaw-Manager"

  on_macos do
    url "https://github.com/lbjlaq/OpsClaw-Manager/releases/download/v#{version}/OpsClaw.Tools_#{version}_universal.dmg"

    app "OpsClaw Tools.app"

    zap trash: [
      "~/Library/Application Support/com.lbjlaq.opsclaw-tools",
      "~/Library/Caches/com.lbjlaq.opsclaw-tools",
      "~/Library/Preferences/com.lbjlaq.opsclaw-tools.plist",
      "~/Library/Saved Application State/com.lbjlaq.opsclaw-tools.savedState",
    ]

    caveats <<~EOS
      If you encounter the "App is damaged" error, please run the following command:
        sudo xattr -rd com.apple.quarantine "/Applications/OpsClaw Tools.app"

      Or install with the --no-quarantine flag:
        brew install --cask --no-quarantine opsclaw-tools
    EOS
  end

  on_linux do
    arch arm: "aarch64", intel: "amd64"

    url "https://github.com/lbjlaq/OpsClaw-Manager/releases/download/v#{version}/OpsClaw.Tools_#{version}_#{arch}.AppImage"
    binary "OpsClaw.Tools_#{version}_#{arch}.AppImage", target: "opsclaw-tools"

    preflight do
      system_command "/bin/chmod", args: ["+x", "#{staged_path}/OpsClaw.Tools_#{version}_#{arch}.AppImage"]
    end
  end
end
