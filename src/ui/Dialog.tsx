import { useEffect, useRef, type ReactNode } from "react";

import { IconButton } from "./Button";

interface DialogProps {
  title: string;
  onClose: () => void;
  /** Keeps the dialog open while work that cannot be cancelled runs. */
  locked?: boolean;
  children: ReactNode;
  footer?: ReactNode;
}

/** A modal sheet. The native element gives focus trapping and Escape. */
export function Dialog({ title, onClose, locked = false, children, footer }: DialogProps) {
  const element = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = element.current;
    if (dialog && !dialog.open) dialog.showModal();
    return () => dialog?.close();
  }, []);

  return (
    <dialog
      ref={element}
      className="dialog"
      aria-labelledby="dialog-title"
      onCancel={(event) => {
        event.preventDefault();
        if (!locked) onClose();
      }}
    >
      <header className="dialog__header">
        <h2 id="dialog-title">{title}</h2>
        <IconButton icon="close" label="Close" onClick={onClose} disabled={locked} />
      </header>
      <div className="dialog__body">{children}</div>
      {footer ? <footer className="dialog__footer">{footer}</footer> : null}
    </dialog>
  );
}
