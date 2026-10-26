// Presets: what a project can take with one click — MCP servers, language servers — and the
// choices of its Model section: compaction shares and reasoning levels. One set for the person,
// kept by the agent on their machine (the setting `presets`) and edited in Bench's gear; each kind
// falls back to the built-in one until it is edited. Bench gives a project's agent the finished
// spec, so a server's projects take this machine's presets too.

import { getSetting, setSetting } from "../config.ts";
import { LSP_PRESETS } from "../lsp-presets.ts";

export interface Presets {
	/** name → { title, spec as in mcp.json } */
	mcp: Record<string, { title: string; spec: unknown }>;
	/** name → { title, spec as in lsp.json } */
	lsp: Record<string, { title: string; spec: unknown }>;
	/** Compaction choices: a share of the model's window, and a name for it. */
	compaction: { at: number; name: string }[];
	/** The reasoning levels offered (as the model's server names them). */
	reasoning: string[];
}

export const DEFAULT_PRESETS: Presets = {
	mcp: {
		context7: { title: "Context7 — libraries' current docs", spec: { url: "https://mcp.context7.com/mcp", headers: { CONTEXT7_API_KEY: "${CONTEXT7_API_KEY}" } } },
		tavily: { title: "Tavily — web search", spec: { url: "https://mcp.tavily.com/mcp/?tavilyApiKey=${TAVILY_API_KEY}" } },
		brave: { title: "Brave Search — web search", spec: { command: "npx", args: ["-y", "@brave/brave-search-mcp-server@2.1.4"], env: { BRAVE_API_KEY: "${BRAVE_API_KEY}" } } },
		github: { title: "GitHub", spec: { url: "https://api.githubcopilot.com/mcp/", headers: { Authorization: "Bearer ${GITHUB_TOKEN}" } } },
	},
	lsp: Object.fromEntries(Object.entries(LSP_PRESETS).map(([k, v]) => [k, { title: v.title, spec: v.spec }])),
	compaction: [
		{ at: 0.6, name: "Assistant" },
		{ at: 0.7, name: "" },
		{ at: 0.8, name: "" },
		{ at: 0.9, name: "Code" },
	],
	reasoning: ["low", "medium", "high", "xhigh"],
};

/** The presets in force: each kind as edited, else the built-in one. */
export function presets(): Presets {
	const own = getSetting("presets") ?? {};
	return {
		mcp: own.mcp && typeof own.mcp === "object" ? own.mcp : DEFAULT_PRESETS.mcp,
		lsp: own.lsp && typeof own.lsp === "object" ? own.lsp : DEFAULT_PRESETS.lsp,
		compaction: Array.isArray(own.compaction) ? own.compaction : DEFAULT_PRESETS.compaction,
		reasoning: Array.isArray(own.reasoning) ? own.reasoning : DEFAULT_PRESETS.reasoning,
	};
}

/** One kind anew (validated); null: back to the built-in one. */
export function savePresets(kind: keyof Presets, value: unknown): void {
	if (value !== null) {
		const ok =
			kind === "mcp" || kind === "lsp"
				? !!value && typeof value === "object" && !Array.isArray(value) && Object.values(value as object).every((p: any) => p && typeof p === "object" && "spec" in p)
				: kind === "compaction"
					? Array.isArray(value) && value.every((c: any) => typeof c?.at === "number" && c.at >= 0.4 && c.at <= 0.95)
					: Array.isArray(value) && value.every((l) => typeof l === "string" && l.trim());
		if (!ok) throw new Error(kind === "compaction" ? "a list of {at: 0.4…0.95, name}" : kind === "reasoning" ? "a list of level names" : "name → {title, spec}");
	}
	const own = { ...(getSetting("presets") ?? {}) };
	if (value === null) delete own[kind];
	else own[kind] = value;
	setSetting("presets", own);
}
