// A language server for tests: every line with ERROR in it is an error, WARN a warning.
import { createMessageConnection, StreamMessageReader, StreamMessageWriter } from "vscode-jsonrpc/node.js";
const c = createMessageConnection(new StreamMessageReader(process.stdin), new StreamMessageWriter(process.stdout));
const publish = (uri, text) =>
	c.sendNotification("textDocument/publishDiagnostics", {
		uri,
		diagnostics: text.split("\n").flatMap((l, line) =>
			l.includes("ERROR") ? [{ range: { start: { line, character: l.indexOf("ERROR") }, end: { line, character: l.length } }, severity: 1, message: "found ERROR", source: "fake" }]
			: l.includes("WARN") ? [{ range: { start: { line, character: 0 }, end: { line, character: 1 } }, severity: 2, message: "a warning", source: "fake" }]
			: []),
	});
c.onRequest("initialize", () => ({ capabilities: { textDocumentSync: 1 } }));
c.onNotification("textDocument/didOpen", (p) => publish(p.textDocument.uri, p.textDocument.text));
c.onNotification("textDocument/didChange", (p) => publish(p.textDocument.uri, p.contentChanges[0].text));
c.listen();
