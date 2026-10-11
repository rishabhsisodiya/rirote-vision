import "./setup.css";

import type { CheckStatus, SetupAction, SetupCheck } from "../lib/types";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Icon, type IconName } from "../ui/Icon";
import { Notice } from "../ui/Notice";
import { Spinner } from "../ui/Spinner";
import type { SetupState } from "./useSetup";

const STATUS: Record<CheckStatus, { icon: IconName; word: string }> = {
  ok: { icon: "check", word: "Ready" },
  warning: { icon: "alert", word: "Check" },
  missing: { icon: "missing", word: "Missing" },
};

interface SetupDialogProps {
  setup: SetupState;
  onCheck: () => void;
  onInstall: (action: SetupAction) => void;
  onClose: () => void;
}

export function SetupDialog({ setup, onCheck, onInstall, onClose }: SetupDialogProps) {
  const { report, checking, installing, error } = setup;
  const busy = checking || installing !== null;
  return (
    <Dialog
      title="Check Setup"
      onClose={onClose}
      locked={installing !== null}
      footer={
        <>
          <p className="setup__summary" role="status">
            {installing
              ? "Installing. This can take a few minutes."
              : checking
                ? "Checking this Mac."
                : report?.ready
                  ? "Everything the app needs is in place."
                  : "Some required items need attention."}
          </p>
          <Button onClick={onCheck} busy={checking} disabled={busy}>
            Check Again
          </Button>
          <Button variant="primary" onClick={onClose} disabled={installing !== null}>
            Done
          </Button>
        </>
      }
    >
      {error ? <Notice message={error} /> : null}
      {report ? (
        <ul className="setup__list">
          {report.checks.map((check) => (
            <CheckRow
              key={check.id}
              check={check}
              installing={installing}
              disabled={busy}
              onInstall={onInstall}
            />
          ))}
        </ul>
      ) : checking ? (
        <p className="setup__loading">
          <Spinner label="Checking this Mac" />
        </p>
      ) : null}
    </Dialog>
  );
}

const INSTALL_LABEL: Record<SetupAction, string> = {
  "install-engine": "Install Video Engine",
  "install-voice": "Install Voiceover and Music",
  "install-skills": "Install Video Skills",
};

interface CheckRowProps {
  check: SetupCheck;
  installing: string | null;
  disabled: boolean;
  onInstall: (action: SetupAction) => void;
}

function CheckRow({ check, installing, disabled, onInstall }: CheckRowProps) {
  const status = STATUS[check.status];
  const { action } = check;
  return (
    <li className={`check check--${check.status}`}>
      <span className="check__status">
        <Icon name={status.icon} />
        <span>{status.word}</span>
      </span>
      <div className="check__text">
        <p className="check__label">
          {check.label}
          {check.required ? null : <span className="check__optional">Optional</span>}
        </p>
        <p className="check__detail">{check.detail}</p>
        {check.hint ? <p className="check__hint">{check.hint}</p> : null}
      </div>
      {action ? (
        <Button
          variant={check.required ? "primary" : "secondary"}
          onClick={() => onInstall(action)}
          busy={installing === action}
          disabled={disabled}
        >
          {installing === action ? "Installing" : INSTALL_LABEL[action]}
        </Button>
      ) : null}
    </li>
  );
}
