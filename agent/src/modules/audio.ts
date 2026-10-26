// Dictation and read-aloud through the stt / tts settings (OpenAI-compatible servers).

import { speak, transcribe } from "../audio.ts";
import { dataUrl, HttpError, profileOf, type Module } from "../http.ts";

export default (): Module => ({
	routes: {
		"POST /api/audio/transcribe": async (req) => {
			try {
				return { transcript: await transcribe(profileOf(req), dataUrl(req.body?.data_url), String(req.body?.mime_type ?? "audio/wav")) };
			} catch (e) {
				throw new HttpError(502, String((e as Error).message));
			}
		},
		"POST /api/audio/speak": async (req) => {
			try {
				const { type, audio } = await speak(profileOf(req), String(req.body?.text ?? ""));
				return { data_url: `data:${type};base64,${audio.toString("base64")}` };
			} catch (e) {
				throw new HttpError(502, String((e as Error).message));
			}
		},
	},
});
