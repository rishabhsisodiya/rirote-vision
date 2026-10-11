import "./preview.css";

import { Button } from "../ui/Button";
import { Spinner } from "../ui/Spinner";
import { Transport } from "./Transport";
import type { PlayerControls } from "./usePlayer";
import type { PreviewState } from "./usePreview";

interface PreviewPaneProps {
  preview: PreviewState;
  player: PlayerControls;
  onRestart: () => void;
  onCheckSetup: () => void;
}

/** The picture and its playback controls. */
export function PreviewPane({ preview, player, onRestart, onCheckSetup }: PreviewPaneProps) {
  if (preview.status === "starting") {
    return (
      <div className="preview__state" role="status">
        <Spinner />
        <p>Starting the preview</p>
      </div>
    );
  }
  if (preview.status === "error") {
    return (
      <div className="preview__state">
        <p className="preview__state-title">The preview is not running</p>
        <p className="preview__state-text">{preview.message}</p>
        {preview.needsSetup ? (
          <Button onClick={onCheckSetup}>Check Setup</Button>
        ) : (
          <Button onClick={onRestart}>Start Preview Again</Button>
        )}
      </div>
    );
  }
  return <Player player={player} onRestart={onRestart} />;
}

function Player({ player, onRestart }: { player: PlayerControls; onRestart: () => void }) {
  const { ready, error } = player.playback;

  return (
    <>
      <div className="preview__picture">
        {player.source ? <hyperframes-player ref={player.attach} src={player.source} /> : null}
        {error ? (
          <div className="preview__state preview__state--over">
            <p className="preview__state-title">The composition could not be shown</p>
            <p className="preview__state-text">{error}</p>
            <Button onClick={onRestart}>Start Preview Again</Button>
          </div>
        ) : !ready ? (
          <div className="preview__state preview__state--over" role="status">
            <Spinner />
            <p>Loading the composition</p>
          </div>
        ) : null}
      </div>
      <Transport player={player} />
    </>
  );
}
