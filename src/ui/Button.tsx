import type { ButtonHTMLAttributes, ReactNode } from "react";

import { Icon, type IconName } from "./Icon";
import { Spinner } from "./Spinner";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost";
  icon?: IconName;
  /** Shows a spinner and blocks clicks while work is under way. */
  busy?: boolean;
  children: ReactNode;
}

export function Button({
  variant = "secondary",
  icon,
  busy = false,
  disabled,
  children,
  className = "",
  type = "button",
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      className={`button button--${variant} ${className}`}
      disabled={disabled || busy}
      aria-busy={busy || undefined}
      {...rest}
    >
      {busy ? <Spinner /> : icon ? <Icon name={icon} /> : null}
      <span>{children}</span>
    </button>
  );
}

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  icon: IconName;
  /** What the button does, for screen readers and the tooltip. */
  label: string;
  pressed?: boolean;
}

export function IconButton({ icon, label, pressed, className = "", ...rest }: IconButtonProps) {
  return (
    <button
      type="button"
      className={`icon-button ${className}`}
      aria-label={label}
      aria-pressed={pressed}
      title={label}
      // A click must not take the keyboard away from the editor: Space still
      // plays after pressing a tool. Tab reaches the button as usual.
      onMouseDown={(event) => event.preventDefault()}
      {...rest}
    >
      <Icon name={icon} />
    </button>
  );
}
