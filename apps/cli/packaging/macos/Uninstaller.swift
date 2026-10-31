import AppKit
import Foundation

func fail(_ message: String) -> Never {
    let alert = NSAlert()
    alert.alertStyle = .critical
    alert.messageText = "Uninstall TermKey"
    alert.informativeText = message
    alert.runModal()
    exit(1)
}

func info(_ message: String) {
    let alert = NSAlert()
    alert.messageText = "Uninstall TermKey"
    alert.informativeText = message
    addVaultCleanupCommand(to: alert)
    alert.runModal()
}

func retainedVaultDirectory() -> URL {
    if let index = CommandLine.arguments.firstIndex(of: "--vault-dir"),
       index + 1 < CommandLine.arguments.count {
        return URL(fileURLWithPath: CommandLine.arguments[index + 1])
    }
    if let directory = ProcessInfo.processInfo.environment["TERMKEY_VAULT_DIR"] {
        return URL(fileURLWithPath: directory)
    }
    let directory = URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent(".termkey")
    if let data = try? Data(contentsOf: directory.appendingPathComponent("config.json")),
       let config = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
       let vaultPath = config["vault_path"] as? String, !vaultPath.isEmpty {
        return URL(fileURLWithPath: vaultPath).deletingLastPathComponent()
    }
    return directory
}

func vaultCleanupCommand(for directory: URL) -> String {
    var names = ["vault.ck", "vault.tmp", "vault.lock", "config.json", "config.tmp", "config.lock"]
    if let entries = try? FileManager.default.contentsOfDirectory(atPath: directory.path) {
        names += entries.filter { $0.hasPrefix(".vault.ck.tmp-") || $0.hasPrefix(".config.json.tmp-") }
    }
    let paths = names.sorted().map { directory.appendingPathComponent($0).path }
    guard paths.allSatisfy({ $0.rangeOfCharacter(from: .controlCharacters) == nil }) else {
        return "Remove vault and settings files manually from the retained vault folder."
    }
    return "rm -f -- " + paths.map(shellQuote).joined(separator: " ")
}

func addVaultCleanupCommand(to alert: NSAlert) {
    let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 520, height: 130))
    scroll.hasVerticalScroller = true
    let text = NSTextView(frame: scroll.bounds)
    text.isEditable = false
    text.isSelectable = true
    text.font = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
    text.isVerticallyResizable = true
    text.textContainer?.widthTracksTextView = true
    text.string = vaultCleanupCommand(for: retainedVaultDirectory())
    scroll.documentView = text
    alert.accessoryView = scroll
}

func confirmUninstall() -> Bool {
    let alert = NSAlert()
    alert.alertStyle = .warning
    alert.messageText = "Remove TermKey from this Mac?"
    alert.informativeText = """
    This removes TermKey.app, the termkey command line tools, and the installer receipt.
    Your vault, settings, and encrypted backups are NOT deleted.
    Retained vault folder: \(retainedVaultDirectory().path)

    To permanently remove the vault and settings afterward, copy the command below. Exported backups are preserved; delete unwanted backups separately.
    """
    addVaultCleanupCommand(to: alert)
    alert.addButton(withTitle: "Uninstall")
    alert.addButton(withTitle: "Cancel")
    return alert.runModal() == .alertFirstButtonReturn
}

func escapeAppleScriptString(_ value: String) -> String {
    value
        .replacingOccurrences(of: "\\", with: "\\\\")
        .replacingOccurrences(of: "\"", with: "\\\"")
}

func shellQuote(_ value: String) -> String {
    "'\(value.replacingOccurrences(of: "'", with: "'\"'\"'"))'"
}

let app = NSApplication.shared
app.setActivationPolicy(.regular)
app.activate(ignoringOtherApps: true)

guard confirmUninstall() else {
    exit(0)
}

let uninstallAppPath = Bundle.main.bundleURL.path
let shellCommand = [
    "/bin/rm -f \(shellQuote("/usr/local/bin/termkey"))",
    "/bin/rm -rf \(shellQuote("/Applications/TermKey.app"))",
    "/usr/sbin/pkgutil --forget \(shellQuote("com.ryanonmars.termkey")) >/dev/null 2>&1 || true",
    "/bin/rm -rf \(shellQuote(uninstallAppPath))",
].joined(separator: "; ")

let scriptSource = """
do shell script "\(escapeAppleScriptString(shellCommand))" with administrator privileges
"""

var error: NSDictionary?
guard let script = NSAppleScript(source: scriptSource) else {
    fail("Could not initialize the uninstall script.")
}

script.executeAndReturnError(&error)

if let error {
    let errorNumber = error[NSAppleScript.errorNumber] as? Int
    if errorNumber == -128 {
        exit(0)
    }

    let message = error[NSAppleScript.errorMessage] as? String ?? "Unknown AppleScript error."
    fail(message)
}

info("TermKey was removed. Your vault, settings, and encrypted backups were left untouched.\n\nRetained vault folder: \(retainedVaultDirectory().path)\n\nTo permanently remove the vault and settings, copy and run the command below. Exported backups are preserved; delete unwanted backups separately.")
