import { useEffect, useMemo, useRef, useState } from "react";

import { shortPath } from "../lib/format";
import type { RecentProject } from "../lib/types";
import { Icon, type IconName } from "../ui/Icon";
import { matches } from "./search";

export interface PaletteCommand {
  id: string;
  label: string;
  icon: IconName;
  keys?: string;
  run: () => void;
}

interface CommandPaletteProps {
  commands: PaletteCommand[];
  recent: RecentProject[];
  onOpenProject: (project: RecentProject) => void;
  onClose: () => void;
}

interface Row {
  key: string;
  label: string;
  detail?: string;
  icon: IconName;
  keys?: string;
  run: () => void;
}

/** ⌘K: find a project or run a command by typing. */
export function CommandPalette({ commands, recent, onOpenProject, onClose }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const dialog = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const element = dialog.current;
    if (element && !element.open) element.showModal();
    return () => element?.close();
  }, []);

  const rows = useMemo<Row[]>(() => {
    const projects: Row[] = recent
      .filter((project) => !project.missing && matches(query, project.name, project.path))
      .slice(0, 8)
      .map((project) => ({
        key: `project:${project.path}`,
        label: project.name,
        detail: shortPath(project.path),
        icon: "folder",
        run: () => onOpenProject(project),
      }));
    const actions: Row[] = commands
      .filter((command) => matches(query, command.label))
      .map((command) => ({ key: command.id, label: command.label, icon: command.icon, keys: command.keys, run: command.run }));
    // With nothing typed, commands lead; once searching, projects do.
    return query ? [...projects, ...actions] : [...actions, ...projects];
  }, [commands, recent, query, onOpenProject]);

  useEffect(() => setActive(0), [query]);

  const choose = (row: Row | undefined) => {
    if (!row) return;
    onClose();
    row.run();
  };

  return (
    <dialog
      ref={dialog}
      className="palette"
      aria-label="Search projects and commands"
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      onClick={(event) => {
        if (event.target === dialog.current) onClose();
      }}
    >
      <div className="palette__field">
        <Icon name="search" />
        <input
          autoFocus
          value={query}
          placeholder="Search projects or run a command"
          aria-label="Search projects or run a command"
          aria-controls="palette-results"
          aria-activedescendant={rows[active] ? `palette-${active}` : undefined}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown") {
              event.preventDefault();
              setActive((index) => Math.min(rows.length - 1, index + 1));
            } else if (event.key === "ArrowUp") {
              event.preventDefault();
              setActive((index) => Math.max(0, index - 1));
            } else if (event.key === "Enter") {
              event.preventDefault();
              choose(rows[active]);
            }
          }}
        />
        <kbd>esc</kbd>
      </div>
      <ul id="palette-results" className="palette__results" role="listbox">
        {rows.length === 0 ? <li className="palette__empty">Nothing matches “{query}”.</li> : null}
        {rows.map((row, index) => (
          <li
            key={row.key}
            id={`palette-${index}`}
            role="option"
            aria-selected={index === active}
            className="palette__row"
            onMouseMove={() => setActive(index)}
            onClick={() => choose(row)}
          >
            <Icon name={row.icon} />
            <span className="palette__label">{row.label}</span>
            {row.detail ? <span className="palette__detail">{row.detail}</span> : null}
            {row.keys ? <kbd>{row.keys}</kbd> : null}
          </li>
        ))}
      </ul>
    </dialog>
  );
}
