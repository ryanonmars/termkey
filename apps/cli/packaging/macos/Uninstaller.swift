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
    alert.runModal()
}

func confirmUninstall() -> Bool {
    let alert = NSAlert()
    alert.alertStyle = .warning
    alert.messageText = "Remove TermKey from this Mac?"
    alert.informativeText = """
    This removes TermKey.app, the termkey command line tools, Chrome integration files installed by TermKey, and the installer receipt.
    Your encrypted vault in ~/.termkey is not deleted.
    """
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
let homeDirectory = NSHomeDirectory()
let chromeNativeHostManifestPath =
    "\(homeDirectory)/Library/Application Support/Google/Chrome/NativeMessagingHosts/com.ryanonmars.termkey.json"
let managedExtensionPath = "\(homeDirectory)/Applications/TermKey Browser Extension"
let legacyExtensionPaths = [
    "\(homeDirectory)/.termkey/browser/chrome-extension",
    "\(homeDirectory)/.cryptokeeper/browser/chrome-extension",
]
let shellCommand = [
    "/bin/rm -f \(shellQuote("/usr/local/bin/termkey"))",
    "/bin/rm -f \(shellQuote("/usr/local/bin/termkey-native-host"))",
    "/bin/rm -f \(shellQuote(chromeNativeHostManifestPath))",
    "/bin/rm -rf \(shellQuote(managedExtensionPath))",
    legacyExtensionPaths.map { "/bin/rm -rf \(shellQuote($0))" }.joined(separator: "; "),
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

info("TermKey and its browser integration files were removed. Your vault data in ~/.termkey was left untouched.")
