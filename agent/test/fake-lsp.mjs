// A language server for tests: every line with ERROR in it is an error, WARN a warning; a hover
// names the place asked; every symbol is defined at the start of the file and used on line 2;
// a file's outline is one class with a method; a symbol search finds `<query>Thing` in the file
// opened last.
import { createMessageConnection, StreamMessageReader, StreamMessageWriter } from "vscode-jsonrpc/node.js";
const c = createMessageConnection(new StreamMessageReader(process.stdin), new StreamMessageWriter(process.stdout));
const publish = (uri, text) => {
	// A text with CRASH in it stands for a server that dies under load (a real crash).
	if (text.includes("CRASH")) process.exit(7);
	return c.sendNotification("textDocument/publishDiagnostics", {
		uri,
		diagnostics: text.split("\n").flatMap((l, line) =>
			l.includes("ERROR") ? [{ range: { start: { line, character: l.indexOf("ERROR") }, end: { line, character: l.length } }, severity: 1, message: "found ERROR", source: "fake" }]
			: l.includes("WARN") ? [{ range: { start: { line, character: 0 }, end: { line, character: 1 } }, severity: 2, message: "a warning", source: "fake" }]
			: []),
	});
};
c.onRequest("initialize", () => ({ capabilities: { textDocumentSync: 1 } }));
let opened = "";
c.onNotification("textDocument/didOpen", (p) => ((opened = p.textDocument.uri), publish(p.textDocument.uri, p.textDocument.text)));
c.onNotification("textDocument/didChange", (p) => publish(p.textDocument.uri, p.contentChanges[0].text));
c.onRequest("textDocument/hover", (p) => ({ contents: { kind: "markdown", value: `at ${p.position.line}:${p.position.character}` } }));
c.onRequest("textDocument/definition", (p) => [{ targetUri: p.textDocument.uri, targetRange: { start: { line: 0, character: 0 }, end: { line: 0, character: 3 } }, targetSelectionRange: { start: { line: 0, character: 0 }, end: { line: 0, character: 3 } } }]);
const at = (line, character = 0) => ({ start: { line, character }, end: { line, character: character + 3 } });
c.onRequest("textDocument/references", (p) => [{ uri: p.textDocument.uri, range: at(0) }, { uri: p.textDocument.uri, range: at(1, 2) }]);
c.onRequest("textDocument/documentSymbol", () => [{ name: "Outer", kind: 5, range: at(0), selectionRange: at(0), children: [{ name: "inner", kind: 6, range: at(1), selectionRange: at(1) }] }]);
c.onRequest("workspace/symbol", (p) => [{ name: `${p.query}Thing`, kind: 12, location: { uri: opened, range: at(1) } }]);
c.listen();
