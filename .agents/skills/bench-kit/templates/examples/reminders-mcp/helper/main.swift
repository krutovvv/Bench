// reminders-helper: the EventKit half of reminders-mcp, inside "Reminders Bridge.app" (a
// background app, so macOS grants Reminders access to it once, whoever runs it). One JSON
// command in, one JSON answer out: `reminders-helper <in.json> <out.json>` (as the MCP runs it
// through `open`), or stdin → stdout. Raw fields only; the MCP (Node) keeps the meaning of the
// notes lines.
//
//   {"op":"list","status":"open|completed|all","from":"YYYY-MM-DD","to":"YYYY-MM-DD"}
//   {"op":"save","id":?, "title","notes","due":"YYYY-MM-DD"|null,"priority":0..9,
//    "completed":"YYYY-MM-DD"|null}
//   {"op":"delete","id"}
//   {"op":"lists"}
import EventKit
import Foundation

let store = EKEventStore()

let args = CommandLine.arguments
let outPath: String? = args.count > 2 ? args[2] : nil

func emit(_ data: Data) {
    if let p = outPath {
        try? data.write(to: URL(fileURLWithPath: p))
    } else {
        FileHandle.standardOutput.write(data)
    }
}

func fail(_ msg: String) -> Never {
    emit(try! JSONSerialization.data(withJSONObject: ["error": msg]))
    exit(1)
}

/// Spin the main run loop until `done()` (EventKit answers there) or the time is up.
func wait(_ seconds: Double, _ done: () -> Bool) -> Bool {
    let end = Date().addingTimeInterval(seconds)
    while !done() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    return done()
}

func access() {
    let status = EKEventStore.authorizationStatus(for: .reminder)
    if status == .fullAccess { return }
    if status == .denied || status == .restricted {
        fail("access to Reminders was denied for this process (System Settings → Privacy & Security → Reminders)")
    }
    var answer: Bool? = nil
    store.requestFullAccessToReminders { granted, _ in DispatchQueue.main.async { answer = granted } }
    if !wait(90, { answer != nil }) || answer != true {
        fail("no access to Reminders (System Settings → Privacy & Security → Reminders)")
    }
}

let dayFormat: DateFormatter = {
    let f = DateFormatter()
    f.calendar = Calendar(identifier: .gregorian)
    f.locale = Locale(identifier: "en_US_POSIX")
    f.dateFormat = "yyyy-MM-dd"
    return f
}()

func day(_ s: Any?) -> Date? { (s as? String).flatMap { dayFormat.date(from: $0) } }

/// Noon of the day, so time zones never move it.
func noon(_ d: Date) -> Date { Calendar.current.date(bySettingHour: 12, minute: 0, second: 0, of: d) ?? d }

func row(_ r: EKReminder) -> [String: Any] {
    var o: [String: Any] = [
        "id": r.calendarItemIdentifier,
        "title": r.title ?? "",
        "notes": r.notes ?? "",
        "priority": r.priority,
        "list": r.calendar?.title ?? "",
    ]
    if let c = r.dueDateComponents, let d = Calendar.current.date(from: c) { o["due"] = dayFormat.string(from: d) }
    if r.isCompleted { o["completed"] = dayFormat.string(from: r.completionDate ?? Date()) }
    if let d = r.creationDate { o["created"] = ISO8601DateFormatter().string(from: d) }
    if let u = r.url { o["url"] = u.absoluteString }
    return o
}

func fetch(_ p: NSPredicate) -> [EKReminder] {
    var out: [EKReminder]? = nil
    store.fetchReminders(matching: p) { found in DispatchQueue.main.async { out = found ?? [] } }
    _ = wait(60, { out != nil })
    return out ?? []
}

let input = args.count > 1
    ? ((try? Data(contentsOf: URL(fileURLWithPath: args[1]))) ?? Data())
    : FileHandle.standardInput.readDataToEndOfFile()
guard let cmd = (try? JSONSerialization.jsonObject(with: input)) as? [String: Any],
      let op = cmd["op"] as? String else { fail("expected a JSON command") }
access()

var answer: Any = [:]
switch op {
case "lists":
    answer = store.calendars(for: .reminder).map { $0.title }
case "list":
    let status = cmd["status"] as? String ?? "open"
    let from = day(cmd["from"]), to = day(cmd["to"]).map { $0.addingTimeInterval(86400) }
    var rows: [EKReminder] = []
    if status != "completed" {
        rows += fetch(store.predicateForIncompleteReminders(withDueDateStarting: nil, ending: nil, calendars: nil))
    }
    if status != "open" {
        rows += fetch(store.predicateForCompletedReminders(withCompletionDateStarting: from, ending: to, calendars: nil))
    }
    answer = rows.map(row)
case "save":
    let r: EKReminder
    if let id = cmd["id"] as? String, !id.isEmpty {
        guard let found = store.calendarItem(withIdentifier: id) as? EKReminder else { fail("no reminder \(id)") }
        r = found
    } else {
        r = EKReminder(eventStore: store)
        guard let list = store.defaultCalendarForNewReminders() else { fail("Reminders has no default list") }
        r.calendar = list
    }
    if let t = cmd["title"] as? String { r.title = t }
    if let n = cmd["notes"] as? String { r.notes = n.isEmpty ? nil : n }
    if cmd.keys.contains("due") {
        if let d = day(cmd["due"]) {
            r.dueDateComponents = Calendar.current.dateComponents([.year, .month, .day], from: d)
        } else { r.dueDateComponents = nil }
    }
    if let p = cmd["priority"] as? Int { r.priority = p }
    if cmd.keys.contains("completed") {
        if let d = day(cmd["completed"]) {
            r.isCompleted = true
            r.completionDate = noon(d)   // any day: back-dating works
        } else { r.isCompleted = false }
    }
    // No alarms are ever added: a task does not ring on the phone unless someone sets one.
    do { try store.save(r, commit: true) } catch { fail(error.localizedDescription) }
    answer = row(r)
case "delete":
    guard let id = cmd["id"] as? String, let r = store.calendarItem(withIdentifier: id) as? EKReminder else {
        fail("no such reminder")
    }
    do { try store.remove(r, commit: true) } catch { fail(error.localizedDescription) }
    answer = ["deleted": id]
default:
    fail("unknown op \(op)")
}
emit(try! JSONSerialization.data(withJSONObject: answer))
