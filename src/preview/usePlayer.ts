import "@hyperframes/player";

import type { HyperframesPlayer } from "@hyperframes/player";
import { useCallback, useEffect, useRef, useState } from "react";

import { clamp } from "../lib/format";
import { logToApp } from "../lib/ipc";

/** Arrow keys step by one frame at this rate. */
const STEP_FPS = 30;
/** For how many seconds of each play the clock is written to the log. */
const PLAY_LOG_SECONDS = 12;
/** Messages the composition sends many times a second; not worth a log line. */
const QUIET_MESSAGES = new Set(["state", "timeline", "ready", "shader-transition-state", "composition_seeked", "stage-size"]);

/** The speeds playback can run at; L goes up the list and J goes down. */
export const RATES = [0.25, 0.5, 1, 2, 4];

export interface Playback {
  /** The composition has loaded and can be played. */
  ready: boolean;
  playing: boolean;
  currentTime: number;
  duration: number;
  muted: boolean;
  looping: boolean;
  /** Playback speed, 1 being real time. */
  rate: number;
  /** Why the composition could not be shown, if it could not. */
  error: string | null;
}

const IDLE: Playback = {
  ready: false,
  playing: false,
  currentTime: 0,
  duration: 0,
  muted: false,
  looping: false,
  rate: 1,
  error: null,
};

/**
 * Drives one <hyperframes-player>. `src` is the composition's URL, or null
 * while there is nothing to play. `revision` changes when the project
 * changes on disk: the composition is then reloaded and parked on the frame
 * it was showing.
 *
 * The element comes and goes with the preview, so it is handed over through
 * `attach` instead of being read from a ref on mount.
 */
export function usePlayer(src: string | null, revision: string) {
  const element = useRef<HyperframesPlayer | null>(null);
  const [mounted, setMounted] = useState<HyperframesPlayer | null>(null);
  const [playback, setPlayback] = useState<Playback>(IDLE);
  /** Where to return to after a reload. */
  const resumeAt = useRef(0);
  /**
   * The last place the playhead was sent, and until when reports from before
   * that seek may still be arriving. The player answers by message, so a
   * report sent just before a seek can land just after it.
   */
  const sought = useRef({ time: 0, until: 0 });

  const attach = useCallback((player: HyperframesPlayer | null) => {
    element.current = player;
    setMounted(player);
  }, []);

  const patch = useCallback((change: Partial<Playback>) => {
    setPlayback((current) => ({ ...current, ...change }));
  }, []);

  useEffect(() => {
    const player = mounted;
    if (!player) return;

    const onReady = (event: Event) => {
      const { duration } = (event as CustomEvent<{ duration: number }>).detail;
      const at = clamp(resumeAt.current, 0, duration);
      if (at > 0) player.seek(at);
      patch({ ready: true, duration, currentTime: at, playing: false, error: null, rate: player.playbackRate });
      logToApp(`preview ready: ${duration.toFixed(2)}s`);
    };
    const onTime = (event: Event) => {
      const { currentTime } = (event as CustomEvent<{ currentTime: number }>).detail;
      const stale =
        performance.now() < sought.current.until &&
        player.paused &&
        Math.abs(currentTime - sought.current.time) > 2 / STEP_FPS;
      if (stale) return;
      resumeAt.current = currentTime;
      patch({ currentTime });
    };
    const onDuration = (event: Event) => {
      patch({ duration: (event as CustomEvent<{ duration: number }>).detail.duration });
    };
    const onPlay = () => patch({ playing: true });
    const onPause = () => patch({ playing: false });
    const onPlaybackError = (event: Event) => {
      const detail = (event as CustomEvent<{ source?: string; error?: unknown }>).detail;
      logToApp(`preview playback error from ${detail?.source ?? "?"}: ${String(detail?.error ?? "")}`);
      patch({ playing: false });
    };
    const onError = (event: Event) => {
      const detail = (event as CustomEvent<{ message?: string }>).detail;
      const message = detail?.message ?? "The composition could not be loaded.";
      patch({ ready: false, playing: false, error: message });
      logToApp(`preview error: ${message}`);
    };

    const handlers: [string, EventListener][] = [
      ["ready", onReady],
      ["timeupdate", onTime],
      ["durationchange", onDuration],
      ["play", onPlay],
      ["pause", onPause],
      ["ended", onPause],
      ["error", onError],
      ["playbackerror", onPlaybackError],
    ];
    for (const [name, handler] of handlers) player.addEventListener(name, handler);
    return () => {
      for (const [name, handler] of handlers) player.removeEventListener(name, handler);
    };
  }, [mounted, patch]);

  // A new revision means new files: load them, keeping the playhead. While
  // the film is playing the reload waits for it to stop, so a file changing
  // in the background does not cut off what is being watched and heard.
  const shown = useRef(revision);
  if (!playback.playing) shown.current = revision;
  const source = src ? `${src}?v=${encodeURIComponent(shown.current)}` : null;
  useEffect(() => {
    patch({ ready: false, playing: false });
  }, [source, patch]);

  // The transport shows what the player is doing, not what it was last told.
  // The player has ways of stopping that fire no event (being sent to a new
  // position is one), so its own state is read back a few times a second.
  useEffect(() => {
    const player = mounted;
    if (!player) return;
    const timer = window.setInterval(() => {
      const playing = player.ready && !player.paused;
      setPlayback((current) => (current.playing === playing ? current : { ...current, playing }));
    }, 200);
    return () => window.clearInterval(timer);
  }, [mounted]);

  // What the real window does during playback, written to the app's log:
  // the clock once a second for the first seconds of each play, and anything
  // the composition reports about its media. This is how a stalled or silent
  // preview is diagnosed without being at the window.
  const isPlaying = playback.playing;
  useEffect(() => {
    const player = mounted;
    if (!player || !isPlaying) return;
    let ticks = 0;
    const timer = window.setInterval(() => {
      ticks += 1;
      logToApp(`preview clock: ${player.currentTime.toFixed(2)}s after ${ticks}s of play, paused=${player.paused}`);
      if (ticks >= PLAY_LOG_SECONDS) window.clearInterval(timer);
    }, 1000);
    return () => window.clearInterval(timer);
  }, [mounted, isPlaying]);

  useEffect(() => {
    let last = "";
    const onMessage = (event: MessageEvent) => {
      const data = event.data as { source?: string; type?: string } | null;
      if (!data || data.source !== "hf-preview" || !data.type || QUIET_MESSAGES.has(data.type)) return;
      const line = `preview says ${JSON.stringify(data).slice(0, 500)}`;
      // The composition repeats itself; once is enough.
      if (line !== last) logToApp(line);
      last = line;
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, []);

  const seek = useCallback(
    (time: number) => {
      const player = element.current;
      if (!player?.ready) return;
      const at = clamp(time, 0, player.duration);
      // The player stops when it is sent somewhere, and does not say so.
      // Scrubbing a film that is playing should leave it playing, so it is
      // started again, unless it was sent to the very end.
      const wasPlaying = !player.paused;
      player.seek(at);
      if (wasPlaying && player.paused && at < player.duration - 1 / STEP_FPS) player.play();
      sought.current = { time: at, until: performance.now() + 250 };
      resumeAt.current = at;
      patch({ currentTime: at, playing: !player.paused });
    },
    [patch],
  );

  const toggle = useCallback(() => {
    const player = element.current;
    if (!player?.ready) return;
    if (!player.paused) return player.pause();
    // Playing from the last frame starts over.
    if (player.currentTime >= player.duration - 1 / STEP_FPS) player.seek(0);
    player.play();
  }, []);

  const step = useCallback(
    (frames: number) => {
      const player = element.current;
      if (!player?.ready) return;
      player.pause();
      seek(player.currentTime + frames / STEP_FPS);
    },
    [seek],
  );

  const setMuted = useCallback(
    (muted: boolean) => {
      if (element.current) element.current.muted = muted;
      patch({ muted });
    },
    [patch],
  );

  const setLooping = useCallback(
    (looping: boolean) => {
      if (element.current) element.current.loop = looping;
      patch({ looping });
    },
    [patch],
  );

  const setRate = useCallback(
    (rate: number) => {
      if (element.current) element.current.playbackRate = rate;
      patch({ rate });
    },
    [patch],
  );

  /** Moves one speed up or down the list of rates. */
  const shiftRate = useCallback(
    (direction: -1 | 1) => {
      const player = element.current;
      if (!player) return;
      const at = RATES.indexOf(player.playbackRate);
      const next = RATES[Math.min(RATES.length - 1, Math.max(0, (at === -1 ? 2 : at) + direction))] ?? 1;
      setRate(next);
    },
    [setRate],
  );

  return { attach, source, playback, seek, toggle, step, setMuted, setLooping, setRate, shiftRate };
}

export type PlayerControls = ReturnType<typeof usePlayer>;
