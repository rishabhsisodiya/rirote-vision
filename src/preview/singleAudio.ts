// Keeps the preview from playing each sound twice in the app's WebKit.
//
// With Web Audio on, HyperFrames 0.9.9 plays a decoded copy of every sound
// and also connects the muted <audio> element to the speakers, counting on
// a muted element to add silence. Chrome does; WebKit does not, so the app
// heard each voice and music bed twice, about 0.4 s apart: an echo.
// Exports are made separately and were never affected.
//
// The player can tell the engine to play sounds through their own elements
// instead, which it does for slideshows. That control is sent after every
// state replay here. Preview-only Web Audio effects (such as the music dipping
// under the voice) are not heard in the preview as a result; exports keep them.
//
// `_replayBridgeState` and `_sendControl` are the player's internals; the
// player is pinned at 0.9.9, and without them this does nothing.

import type { HyperframesPlayer } from "@hyperframes/player";

interface PlayerInternals {
  _replayBridgeState?: () => void;
  _sendControl?: (action: string, data?: Record<string, unknown>) => boolean;
  __singleAudio?: boolean;
}

export function playSoundsOnce(player: HyperframesPlayer): void {
  const internals = player as unknown as PlayerInternals;
  const replay = internals._replayBridgeState;
  const send = internals._sendControl;
  if (internals.__singleAudio || typeof replay !== "function" || typeof send !== "function") return;
  internals.__singleAudio = true;
  internals._replayBridgeState = function (this: HyperframesPlayer) {
    replay.call(this);
    send.call(this, "set-web-audio-media-disabled", { disabled: true });
  };
}
