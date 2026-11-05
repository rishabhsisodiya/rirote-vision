import { convertFileSrc } from "@tauri-apps/api/core";
import { useState } from "react";

import type { Reference } from "../lib/types";
import { Icon } from "../ui/Icon";

/**
 * The reference video, to watch in the app. Shows its still with a play
 * button first, so nothing loads or plays until it is asked for.
 */
export function ReferenceVideo({ reference, autoPlay = false }: { reference: Reference; autoPlay?: boolean }) {
  const [playing, setPlaying] = useState(autoPlay);
  const [broken, setBroken] = useState(false);
  if (!reference.video) return null;
  const portrait = (reference.height ?? 0) > (reference.width ?? 0);
  const poster = reference.poster ? convertFileSrc(reference.poster) : undefined;

  if (broken) {
    return <p className="side__empty">This video cannot be played here. Its file may have moved; study it again.</p>;
  }
  return (
    <div className={`reference-video ${portrait ? "reference-video--portrait" : ""}`}>
      {playing ? (
        <video
          src={convertFileSrc(reference.video)}
          poster={poster}
          controls
          autoPlay
          playsInline
          onError={() => setBroken(true)}
        />
      ) : (
        <button type="button" className="reference-video__start" aria-label="Play the reference video" onClick={() => setPlaying(true)}>
          {poster ? <img src={poster} alt="" /> : null}
          <span className="reference-video__play">
            <Icon name="play" size={22} />
          </span>
        </button>
      )}
    </div>
  );
}
