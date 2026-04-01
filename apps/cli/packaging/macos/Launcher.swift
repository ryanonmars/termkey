import AppKit
import Foundation

func fail(_ message: String) -> Never {
    let alert = NSAlert()
    alert.messageText = "TermKey"
    alert.informativeText = message
    alert.runModal()
    exit(1)
}

func escapeAppleScriptString(_ value: String) -> String {
    value
        .replacingOccurrences(of: "\\", with: "\\\\")
        .replacingOccurrences(of: "\"", with: "\\\"")
}

let bundleURL = Bundle.main.bundleURL
let binaryURL = bundleURL
    .appendingPathComponent("Contents")
    .appendingPathComponent("Resources")
    .appendingPathComponent("bin")
    .appendingPathComponent("termkey")

guard FileManager.default.isExecutableFile(atPath: binaryURL.path) else {
    fail("The bundled termkey binary is missing.")
}

let command = "clear; exec \(binaryURL.path)"
let escapedCommand = escapeAppleScriptString(command)
let scriptSource = """
tell application "Terminal"
  activate
  do script "\(escapedCommand)"
end tell
"""

var error: NSDictionary?
guard let script = NSAppleScript(source: scriptSource) else {
    fail("Could not initialize the Terminal launcher script.")
}

script.executeAndReturnError(&error)

if let error {
    let message = error[NSAppleScript.errorMessage] as? String ?? "Unknown AppleScript error."
    fail(message)
}
