// `resolve_comment`: the person's review comments on the project's code come in a message as
// "Comment <id> on `path:lines`: <note>" with the lines fenced under it (Bench's ⌘⇧L,
// src/explorer/pane/comment.rs). The agent addresses each and closes it with this call. The call is
// the record (docs/thread-objects.md): Bench reads which comments are resolved, and how, from the
// journal; nothing is kept beside it.

import { Type } from "@earendil-works/pi-ai";
import { defineTool } from "@earendil-works/pi-durable";

const text = (t: unknown) => ({ output: [{ type: "text" as const, text: typeof t === "string" ? t : JSON.stringify(t) }] });

export const resolveComment = defineTool({
	name: "resolve_comment",
	description:
		"Close one of the person's review comments (a message line \"Comment <id> on `path:lines`: <note>\", the lines under it as they were when written). " +
		"Address each comment first — edit the files with your normal tools (the fenced lines are an excerpt; read the file, it may have changed since), or answer when no change is needed — then call this with its id and one line on what you did. " +
		"A comment that is unclear, or that you disagree with: say so in your answer and leave it open.",
	parameters: Type.Object({
		id: Type.String({ description: "The comment's id, e.g. c3f1" }),
		note: Type.String({ description: "One line: what was done" }),
	}),
	replay: "safe",
	execute: async (args) => text({ resolved: args.id }),
});
