import { Icon, type IconName } from "../ui/Icon";

/** Stands in for a pane whose feature is not built yet, and says so. */
export function PanePlaceholder({ icon, title, text }: { icon: IconName; title: string; text: string }) {
  return (
    <div className="placeholder">
      <Icon name={icon} size={22} />
      <p className="placeholder__title">{title}</p>
      <p className="placeholder__text">{text}</p>
    </div>
  );
}
