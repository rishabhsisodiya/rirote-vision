"""Synthesises an original instrumental music bed for RIROTE VISION.

  python music_bed.py <style> <seconds> <out.wav> [impact-second]

Styles "calm", "steady" and "bright" are loops: soft chord pads, a plucked
arpeggio and a low kick, with most of their energy below the speech band so
they sit under a voice. "cinematic" is a score for an unnarrated piece: a low
drone, a pulse and a rhythm that build to one impact at the given second, then
a held chord.

Everything is made from sine waves and noise here, so each file is original
and free to publish. Needs numpy.
"""
import sys
import wave

import numpy as np

SR = 44100
STYLES = {
    # tempo, chord loop (MIDI notes, one chord per bar), pluck level, kick level
    "calm": dict(bpm=88, chords=[[45, 52, 57, 60], [41, 48, 53, 57], [45, 52, 57, 60], [43, 50, 55, 59]], pluck=0.13, kick=0.38),
    "steady": dict(bpm=100, chords=[[50, 57, 62, 65], [46, 53, 58, 62], [41, 53, 57, 60], [48, 55, 60, 64]], pluck=0.20, kick=0.50),
    "bright": dict(bpm=104, chords=[[48, 55, 60, 64], [43, 55, 59, 62], [45, 52, 57, 60], [41, 53, 57, 60]], pluck=0.22, kick=0.42),
}
MIN_SECONDS, MAX_SECONDS = 3.0, 600.0
# The riser before the impact is 2.4 s long and the chord after it needs room to ring.
MIN_BEFORE_IMPACT, MIN_AFTER_IMPACT = 3.0, 2.0


def hz(note):
    return 440.0 * 2 ** ((note - 69) / 12)


def add(buf, start, sig, gain_l=1.0, gain_r=1.0):
    i = int(start * SR)
    if i >= buf.shape[1]:
        return
    n = min(len(sig), buf.shape[1] - i)
    buf[0, i:i + n] += sig[:n] * gain_l
    buf[1, i:i + n] += sig[:n] * gain_r


def pad(note, dur):
    t = np.arange(int(dur * SR)) / SR
    f = hz(note)
    left = np.sin(2 * np.pi * f * 0.997 * t) + 0.35 * np.sin(2 * np.pi * f * 2 * t)
    right = np.sin(2 * np.pi * f * 1.003 * t) + 0.35 * np.sin(2 * np.pi * f * 2.004 * t)
    env = np.minimum(1, t / 0.9) * np.minimum(1, (dur - t) / 0.9)
    return left * env, right * env


def pluck(note, dur=0.55):
    t = np.arange(int(dur * SR)) / SR
    f = hz(note)
    tone = np.sin(2 * np.pi * f * t) + 0.4 * np.sin(2 * np.pi * 2 * f * t) * np.exp(-t * 14) + 0.15 * np.sin(2 * np.pi * 3 * f * t) * np.exp(-t * 22)
    return tone * np.exp(-t * 7.5) * np.minimum(1, t / 0.004)


def kick(dur=0.32):
    t = np.arange(int(dur * SR)) / SR
    freq = 46 + 70 * np.exp(-t * 26)
    return np.sin(2 * np.pi * np.cumsum(freq) / SR) * np.exp(-t * 11)


def bed(style, seconds):
    p = STYLES[style]
    beat = 60.0 / p["bpm"]
    bar = beat * 4
    buf = np.zeros((2, int(seconds * SR)))
    bars = int(np.ceil(seconds / bar)) + 1
    for b in range(bars):
        chord = p["chords"][b % len(p["chords"])]
        t0 = b * bar
        for note in chord:
            left, right = pad(note, bar + 0.9)
            add(buf, t0, left * 0.11, 1, 0)
            add(buf, t0, right * 0.11, 0, 1)
        add(buf, t0, pad(chord[0] - 12, bar + 0.9)[0] * 0.16)
        if b == 0:
            continue  # the first bar is pads only, so the voice opens on a clean bed
        order = [0, 2, 1, 3, 2, 1, 3, 2]
        for k, idx in enumerate(order):
            note = chord[idx] + 12
            hit = pluck(note) * p["pluck"] * (1.0 if k % 2 == 0 else 0.7)
            pan = 0.35 if k % 2 else -0.35
            add(buf, t0 + k * beat / 2, hit, 1 - max(0, pan), 1 + min(0, pan))
            add(buf, t0 + k * beat / 2 + beat * 0.75, hit * 0.3, 1 + min(0, pan), 1 - max(0, pan))  # echo, opposite side
        for k in (0, 2):
            add(buf, t0 + k * beat, kick() * p["kick"])
    fade_in, fade_out = int(0.4 * SR), int(1.4 * SR)
    buf[:, :fade_in] *= np.linspace(0, 1, fade_in)
    buf[:, -fade_out:] *= np.linspace(1, 0, fade_out) ** 2
    return buf / np.max(np.abs(buf)) * 0.7


def cinematic(seconds, impact):
    """Drone and pulse from silence, rhythm joining in, a riser into one impact, then a held chord."""
    n = int(seconds * SR)
    t = np.arange(n) / SR
    buf = np.zeros((2, n))
    rise = np.clip(t / impact, 0, 1)
    after = (t >= impact).astype(float)
    # Low drone on D, swelling towards the impact.
    drone = np.sin(2 * np.pi * 36.71 * t) * 0.55 + np.sin(2 * np.pi * 73.42 * t) * 0.3 + np.sin(2 * np.pi * 110.0 * t) * 0.1 * rise
    drone *= (0.35 + 0.65 * rise ** 1.5) * (1 - 0.35 * after)
    buf += drone * 0.5
    # Slow chords underneath, brighter as the film builds.
    chords = [[50, 57, 62, 65], [46, 53, 58, 62], [41, 53, 57, 60], [48, 55, 60, 64]]
    beat = 60.0 / 100
    bar = beat * 4
    for b in range(int(np.ceil(seconds / bar)) + 1):
        t0 = b * bar
        level = 0.03 + 0.07 * min(1.0, t0 / impact)
        for note in chords[b % 4]:
            left, right = pad(note, bar + 0.9)
            add(buf, t0, left * level, 1, 0)
            add(buf, t0, right * level, 0, 1)
    # Digital pulse: a soft high blip on every beat, with an echo on the other side.
    k = 0
    while 1.0 + k * beat < impact:
        t0 = 1.0 + k * beat
        blip = pluck(86 if k % 4 else 81, 0.3) * (0.05 + 0.07 * t0 / impact)
        add(buf, t0, blip, 1.0 if k % 2 else 0.5, 0.5 if k % 2 else 1.0)
        add(buf, t0 + beat * 0.5, blip * 0.35, 0.5 if k % 2 else 1.0, 1.0 if k % 2 else 0.5)
        k += 1
    # Rhythm: half-time kicks from the second scene, every beat through the montage.
    k = 0
    while 4.0 + k * beat < impact - 0.3:
        t0 = 4.0 + k * beat
        if t0 >= 8.0 or k % 2 == 0:
            add(buf, t0, kick() * (0.35 + 0.3 * t0 / impact))
        k += 1
    # Riser: filtered noise swelling over the last 2.4 s before the impact.
    rng = np.random.default_rng(7)
    length = int(2.4 * SR)
    noise = np.diff(rng.standard_normal(length + 1))
    ramp = np.linspace(0, 1, length)
    add(buf, impact - 2.4, noise * ramp ** 3 * 0.09)
    sweep = np.sin(2 * np.pi * np.cumsum(180 + 900 * ramp ** 2) / SR) * ramp ** 3 * 0.10
    add(buf, impact - 2.4, sweep)
    # Impact: a long low boom and a bright chord that holds to the end.
    ti = np.arange(int(2.6 * SR)) / SR
    boom = np.sin(2 * np.pi * np.cumsum(38 + 60 * np.exp(-ti * 9)) / SR) * np.exp(-ti * 1.6)
    add(buf, impact, boom * 0.95)
    hold = max(1.0, seconds - impact)
    for note in [38, 50, 57, 62, 66, 69]:
        left, right = pad(note, hold)
        swell = np.minimum(1, np.arange(len(left)) / (0.08 * SR))
        add(buf, impact, left * swell * 0.09, 1, 0)
        add(buf, impact, right * swell * 0.09, 0, 1)
    fade_in, fade_out = int(1.2 * SR), int(1.6 * SR)
    buf[:, :fade_in] *= np.linspace(0, 1, fade_in) ** 2
    buf[:, -fade_out:] *= np.linspace(1, 0, fade_out) ** 2
    return buf / np.max(np.abs(buf)) * 0.8



def fail(message):
    print(message, file=sys.stderr)
    sys.exit(2)


def main():
    if len(sys.argv) not in (4, 5):
        fail("usage: music_bed.py <style> <seconds> <out.wav> [impact-second]")
    style, out = sys.argv[1], sys.argv[3]
    try:
        seconds = float(sys.argv[2])
        impact = float(sys.argv[4]) if len(sys.argv) == 5 else None
    except ValueError:
        fail("The length and the impact second must be numbers.")
    if not MIN_SECONDS <= seconds <= MAX_SECONDS:
        fail(f"A music bed can be {MIN_SECONDS:g} to {MAX_SECONDS:g} seconds long.")

    if style == "cinematic":
        if impact is None:
            fail("The cinematic style needs the second where the impact lands.")
        if not MIN_BEFORE_IMPACT <= impact <= seconds - MIN_AFTER_IMPACT:
            fail(f"The impact must be at least {MIN_BEFORE_IMPACT:g} seconds in and {MIN_AFTER_IMPACT:g} seconds before the end.")
        track = cinematic(seconds, impact)
    elif style in STYLES:
        track = bed(style, seconds)
    else:
        fail(f"Unknown style: {style}. Choose calm, steady, bright or cinematic.")

    pcm = (track.T * 32767).astype("<i2")
    with wave.open(out, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm.tobytes())


if __name__ == "__main__":
    main()
