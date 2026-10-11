import { useState } from "react";

import type { TrustLevel } from "../lib/types";
import { Button } from "../ui/Button";
import { DEFAULT_TRUST, TRUST_OPTIONS } from "./trust";

interface TrustChooserProps {
  /** The level in force, when this is a change and not a first choice. */
  current: TrustLevel | null;
  onChoose: (level: TrustLevel) => void;
  onCancel?: () => void;
}

/** Asks, once per project, how much the assistant may do without asking. */
export function TrustChooser({ current, onChoose, onCancel }: TrustChooserProps) {
  const [level, setLevel] = useState<TrustLevel>(current ?? DEFAULT_TRUST);
  return (
    <form
      className="trust"
      onSubmit={(event) => {
        event.preventDefault();
        onChoose(level);
      }}
    >
      <fieldset>
        <legend>How much may the assistant do in this project without asking?</legend>
        {TRUST_OPTIONS.map((option) => (
          <label key={option.value} className={`trust__option trust__option--${option.value}`}>
            <input
              type="radio"
              name="trust"
              checked={level === option.value}
              onChange={() => setLevel(option.value)}
            />
            <span className="trust__label">{option.label}</span>
            <span className="trust__detail">{option.allows}</span>
            <span className="trust__detail trust__detail--asks">{option.asks}</span>
          </label>
        ))}
      </fieldset>
      <p className="trust__note">
        You can change this later from the top of this pane. Your own deny rules in Claude Code
        apply at every level.
      </p>
      <div className="trust__actions">
        {onCancel ? <Button onClick={onCancel}>Cancel</Button> : null}
        <Button variant="primary" type="submit">
          {current ? "Use This Level" : "Continue"}
        </Button>
      </div>
    </form>
  );
}
