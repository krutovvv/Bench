#!/usr/bin/env python3
"""Bench's sounds, written to assets/sounds/ (src/media/sound.rs plays them). Soft struck notes: a sine
with a little of its fourth harmonic, a quick attack and a decay, far below full scale.

    python3 scripts/make-sounds.py
"""

import os
import wave

import numpy as np

RATE = 44100
OUT = os.path.join(os.path.dirname(__file__), "..", "assets", "sounds")


def note(hz, seconds, decay, bright=0.12):
    t = np.arange(int(RATE * seconds)) / RATE
    body = np.sin(2 * np.pi * hz * t) + bright * np.exp(-t * decay * 3) * np.sin(2 * np.pi * hz * 4 * t)
    attack = np.minimum(t / 0.004, 1)
    release = np.minimum((seconds - t) / 0.02, 1)
    return body * attack * release * np.exp(-t * decay)


def notes(*parts):
    """(start in seconds, hz, seconds, decay, loudness), mixed."""
    end = max(start + seconds for start, _, seconds, _, _ in parts)
    mix = np.zeros(int(RATE * end) + 1)
    for start, hz, seconds, decay, loud in parts:
        n = note(hz, seconds, decay) * loud
        at = int(RATE * start)
        mix[at : at + len(n)] += n
    return mix


def save(name, samples, peak):
    samples = samples / np.abs(samples).max() * peak
    with wave.open(os.path.join(OUT, f"{name}.wav"), "wb") as f:
        f.setnchannels(1)
        f.setsampwidth(2)
        f.setframerate(RATE)
        f.writeframes((samples * 32767).astype("<i2").tobytes())


# The mic: two quick notes up, the same two down. The quietest: the mic may hear them.
save("mic_on", notes((0, 660, 0.09, 30, 0.8), (0.055, 880, 0.11, 28, 1)), 0.09)
save("mic_off", notes((0, 880, 0.09, 30, 0.8), (0.055, 660, 0.12, 28, 1)), 0.09)
# A message sent: one short note sliding up, a soft pop.
t = np.arange(int(RATE * 0.09)) / RATE
slide = np.sin(2 * np.pi * np.cumsum(520 + 500 * t / 0.09) / RATE)
save("message_sent", slide * np.minimum(t / 0.004, 1) * np.minimum((0.09 - t) / 0.015, 1) * np.exp(-t * 26), 0.1)
