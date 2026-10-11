import { useCallback, useEffect, useState } from "react";

import { api, errorMessage } from "../lib/ipc";
import type { SetupAction, SetupReport } from "../lib/types";

const INSTALLERS: Record<SetupAction, () => Promise<SetupReport>> = {
  "install-engine": api.engineInstall,
  "install-voice": api.voiceInstall,
  "install-skills": api.skillsInstall,
};

export interface SetupState {
  /** Null until the first check finishes. */
  report: SetupReport | null;
  checking: boolean;
  /** The check being repaired, by its action name. */
  installing: string | null;
  error: string | null;
}

/** The machine's readiness: checked on launch, re-checked or repaired on request. */
export function useSetup() {
  const [state, setState] = useState<SetupState>({
    report: null,
    checking: true,
    installing: null,
    error: null,
  });

  const check = useCallback(async () => {
    setState((current) => ({ ...current, checking: true, error: null }));
    try {
      const report = await api.environmentCheck();
      setState((current) => ({ ...current, report, checking: false }));
    } catch (cause) {
      setState((current) => ({ ...current, checking: false, error: errorMessage(cause) }));
    }
  }, []);

  /** Runs the repair a check offers, then shows the setup as it is afterwards. */
  const install = useCallback(async (action: SetupAction) => {
    setState((current) => ({ ...current, installing: action, error: null }));
    try {
      const report = await INSTALLERS[action]();
      setState((current) => ({ ...current, report, installing: null }));
    } catch (cause) {
      setState((current) => ({ ...current, installing: null, error: errorMessage(cause) }));
    }
  }, []);

  useEffect(() => {
    void check();
  }, [check]);

  return { ...state, check, install };
}
