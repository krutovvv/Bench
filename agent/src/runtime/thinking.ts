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

