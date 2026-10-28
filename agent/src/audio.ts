// Speech in and out through the `stt` / `tts` settings (OpenAI-compatible servers): Bench's
// dictation and read-aloud, and Telegram's voice messages.

import { readConfig, readEnv } from "./config.ts";

function endpoint(agent: string, section: "stt" | "tts") {
	const cfg = readConfig(agent)?.[section] ?? {};
	const o = cfg[cfg.provider ?? "openai"] ?? cfg.openai ?? {};
	const env = readEnv(agent);
	return {
		o,
		base: String(o.base_url ?? "https://api.openai.com/v1").replace(/\/$/, ""),
		key: o.api_key || env.VOICE_TOOLS_OPENAI_KEY || env.OPENAI_API_KEY || "",
	};
}

// fetch says only "fetch failed"; the reason (refused, unreachable, timed out) is in its cause.
// A server on the local network that answers everyone else but not the agent is macOS's Local
// Network privacy: the agent runs under the Bench app that started it, which needs the grant.
async function post(url: string, init: RequestInit): Promise<Response> {
	try {
		return await fetch(url, init);
	} catch (e) {
		const c: any = (e as any)?.cause;
		const why = c?.code ?? c?.message ?? (e as Error).message;
		const host = new URL(url).host;
		const local = /^(10|127|192\.168|172\.(1[6-9]|2\d|3[01]))\.|\.local(:|$)/.test(host);
		const hint = local && /EHOSTUNREACH|ENETUNREACH|EPERM|EACCES/.test(String(why))
			? " (allow Bench in System Settings → Privacy & Security → Local Network)"
			: "";
		throw new Error(`${host}: ${why}${hint}`);
	}
}

export async function transcribe(agent: string, audio: Buffer, mime: string, filename = mime.includes("wav") ? "audio.wav" : "audio.ogg"): Promise<string> {
	const { o, base, key } = endpoint(agent, "stt");
	const form = new FormData();
	form.append("file", new Blob([new Uint8Array(audio)], { type: mime }), filename);
	form.append("model", String(o.model ?? "whisper-1"));
	if (o.language) form.append("language", String(o.language));
	const r = await post(`${base}/audio/transcriptions`, { method: "POST", headers: key ? { Authorization: `Bearer ${key}` } : {}, body: form });
	if (!r.ok) throw new Error(`transcription failed: HTTP ${r.status} ${(await r.text()).slice(0, 200)}`);
	const j: any = await r.json();
	return String(j.text ?? "").trim();
}

export async function speak(agent: string, text: string): Promise<{ type: string; audio: Buffer }> {
	const { o, base, key } = endpoint(agent, "tts");
	const r = await post(`${base}/audio/speech`, {
		method: "POST",
		headers: { "Content-Type": "application/json", ...(key ? { Authorization: `Bearer ${key}` } : {}) },
		body: JSON.stringify({ model: o.model ?? "tts-1", voice: o.voice ?? "alloy", input: text, response_format: "mp3" }),
	});
	if (!r.ok) throw new Error(`speech failed: HTTP ${r.status} ${(await r.text()).slice(0, 200)}`);
	return { type: r.headers.get("content-type") || "audio/mpeg", audio: Buffer.from(await r.arrayBuffer()) };
}
