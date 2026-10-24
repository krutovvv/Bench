// Window id of a process's largest on-screen window, for `screencapture -l <id>`.
// Usage: swift winid.swift <pid>
import CoreGraphics

let pid = Int(CommandLine.arguments[1])!
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
var best = (0, 0.0)
for w in list {
    if let p = w["kCGWindowOwnerPID"] as? Int, p == pid, let n = w["kCGWindowNumber"] as? Int,
       let b = w["kCGWindowBounds"] as? [String: Any], let h = b["Height"] as? Double,
       let wd = b["Width"] as? Double, h * wd > best.1 {
        best = (n, h * wd)
    }
}
print(best.0)
