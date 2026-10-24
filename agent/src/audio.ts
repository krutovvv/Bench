// Speech in and out through Hermes' `stt:` / `tts:` config (OpenAI-compatible servers): Bench's
// dictation and read-aloud, and Telegram's voice messages.

import { readConfig, readEnv } from "./config.ts";

function endpoint(profile: string, section: "stt" | "tts") {
	const cfg = readConfig(profile)?.[section] ?? {};
	const o = cfg[cfg.provider ?? "openai"] ?? cfg.openai ?? {};
	const env = readEnv(profile);
	return {
		o,
		base: String(o.base_url ?? "https://api.openai.com/v1").replace(/\/$/, ""),
		key: o.api_key || env.VOICE_TOOLS_OPENAI_KEY || env.OPENAI_API_KEY || "",
	};
}

export async function transcribe(profile: string, audio: Buffer, mime: string, filename = mime.includes("wav") ? "audio.wav" : "audio.ogg"): Promise<string> {
	const { o, base, key } = endpoint(profile, "stt");
	const form = new FormData();
	form.append("file", new Blob([audio], { type: mime }), filename);
	form.append("model", String(o.model ?? "whisper-1"));
	if (o.language) form.append("language", String(o.language));
	const r = await fetch(`${base}/audio/transcriptions`, { method: "POST", headers: key ? { Authorization: `Bearer ${key}` } : {}, body: form });
	if (!r.ok) throw new Error(`transcription failed: HTTP ${r.status} ${(await r.text()).slice(0, 200)}`);
	const j: any = await r.json();
	return String(j.text ?? "").trim();
}

export async function speak(profile: string, text: string): Promise<{ type: string; audio: Buffer }> {
	const { o, base, key } = endpoint(profile, "tts");
	const r = await fetch(`${base}/audio/speech`, {
		method: "POST",
		headers: { "Content-Type": "application/json", ...(key ? { Authorization: `Bearer ${key}` } : {}) },
		body: JSON.stringify({ model: o.model ?? "tts-1", voice: o.voice ?? "alloy", input: text, response_format: "mp3" }),
	});
	if (!r.ok) throw new Error(`speech failed: HTTP ${r.status} ${(await r.text()).slice(0, 200)}`);
	return { type: r.headers.get("content-type") || "audio/mpeg", audio: Buffer.from(await r.arrayBuffer()) };
}
