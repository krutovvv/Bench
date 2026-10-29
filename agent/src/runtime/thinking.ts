// What turns reasoning off for a request that needs none (titles, compaction).

/**
 * A request with no reasoning asked for, made so on an OpenAI-compatible server that thinks by
 * default (vLLM, SGLang, llama.cpp serving Qwen and the like): Pi sends nothing there unless the
 * model maps "off", so the template's switch goes in (`chat_template_kwargs.enable_thinking`).
 * OpenRouter and OpenAI get Pi's own "off", Anthropic's API thinks only when asked.
 */
export function noThinking(payload: any, model: { api?: string; baseUrl?: string; provider?: string }): any {
	const url = model.baseUrl ?? "";
	if (model.api !== "openai-completions" || /openrouter\.ai|api\.openai\.com/.test(url) || payload?.reasoning_effort) return undefined;
	return { ...payload, chat_template_kwargs: { ...payload?.chat_template_kwargs, enable_thinking: false } };
}

/**
 * The history's answers that carry no reasoning, rendered as they were generated. A request without
 * reasoning is prompted with an empty think block (`<think>\n\n</think>\n\n`, the template's
 * switch); the answer comes back without reasoning, and the template renders it in the next
 * request with no block at all — the prompt differs right there, and the model server's cache falls
 * back to an older checkpoint (measured on the dev server: 4 096 of 7 980 tokens reused, against
 * 7 936 with this). An empty reasoning ("\n", which the template strips) renders the same empty
 * block. Only where `noThinking` applies: an OpenAI-compatible server that thinks by default.
 */
export function emptyThinking(payload: any, model: { api?: string; baseUrl?: string }): any {
	const url = model.baseUrl ?? "";
	if (model.api !== "openai-completions" || /openrouter\.ai|api\.openai\.com/.test(url) || !Array.isArray(payload?.messages)) return undefined;
	let changed = false;
	const messages = payload.messages.map((m: any) => {
		if (m?.role !== "assistant" || m.reasoning_content || m.reasoning || m.reasoning_text) return m;
		changed = true;
		return { ...m, reasoning_content: "\n" };
	});
	return changed ? { ...payload, messages } : undefined;
}
