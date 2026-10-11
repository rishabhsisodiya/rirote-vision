import { IconButton } from "./Button";
import { Icon } from "./Icon";

/** An error shown in the window, where the person is looking, instead of an alert. */
export function Notice({ message, onDismiss }: { message: string; onDismiss?: () => void }) {
  return (
    <div className="notice" role="alert">
      <Icon name="alert" />
      <p>{message}</p>
      {onDismiss ? <IconButton icon="close" label="Dismiss" onClick={onDismiss} /> : null}
    </div>
  );
}
