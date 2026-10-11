import "./shortcuts.css";

import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { SHORTCUT_GROUPS } from "./shortcuts";

/** The list of keyboard shortcuts. */
export function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  return (
    <Dialog
      title="Keyboard Shortcuts"
      onClose={onClose}
      footer={
        <Button variant="primary" onClick={onClose}>
          Done
        </Button>
      }
    >
      <div className="shortcuts">
        {SHORTCUT_GROUPS.map((group) => (
          <section key={group.title}>
            <h3>{group.title}</h3>
            <dl>
              {group.shortcuts.map((shortcut) => (
                <div key={shortcut.does} className="shortcuts__row">
                  <dt>{shortcut.does}</dt>
                  <dd>
                    {shortcut.keys.map((key) => (
                      <kbd key={key}>{key}</kbd>
                    ))}
                  </dd>
                </div>
              ))}
            </dl>
          </section>
        ))}
      </div>
    </Dialog>
  );
}
