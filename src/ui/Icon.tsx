const PATHS = {
  plus: "M12 5v14M5 12h14",
  folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z",
  check: "M5 12.5l4.5 4.5L19 7.5",
  alert: "M12 8v5M12 16.5v.5M10.3 3.9 2.6 17a2 2 0 0 0 1.7 3h15.4a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z",
  missing: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18ZM9 9l6 6M15 9l-6 6",
  close: "M6 6l12 12M18 6 6 18",
  sidebar: "M4 5h16a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1ZM9 5v14",
  assistant: "M4 5h16a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1ZM15 5v14",
  play: "M8 5.5v13l11-6.5Z",
  pause: "M8 5.5v13M16 5.5v13",
  toStart: "M6 5.5v13M19 6v12l-9-6Z",
  toEnd: "M18 5.5v13M5 6v12l9-6Z",
  stepBack: "M15 6.5v11l-8-5.5Z",
  stepForward: "M9 6.5v11l8-5.5Z",
  loop: "M17 3l3 3-3 3M20 6H8a4 4 0 0 0-4 4v1M7 21l-3-3 3-3M4 18h12a4 4 0 0 0 4-4v-1",
  sound: "M4 9.5h3l5-4v13l-5-4H4ZM16 9a4 4 0 0 1 0 6M18.5 6.5a8 8 0 0 1 0 11",
  muted: "M4 9.5h3l5-4v13l-5-4H4ZM16 9.5l5 5M21 9.5l-5 5",
  timeline: "M3 7h11M3 12h18M3 17h7M17 5v4M13 15v4",
  undo: "M9 7 4 12l5 5M4 12h10a6 6 0 0 1 0 12h-1",
  redo: "M15 7l5 5-5 5M20 12H10a6 6 0 0 0 0 12h1",
  split: "M12 3v18M4 8h5v8H4ZM15 8h5v8h-5Z",
  duplicate: "M9 9h11v11H9ZM5 15H4V4h11v1",
  trash: "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3",
  zoomIn: "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14ZM20 20l-4-4M11 8v6M8 11h6",
  zoomOut: "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14ZM20 20l-4-4M8 11h6",
  fit: "M4 9V5h4M20 9V5h-4M4 15v4h4M20 15v4h-4",
  send: "M12 19V5M5.5 11.5 12 5l6.5 6.5",
  stop: "M7 7h10v10H7Z",
  compose: "M12 5H6a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-6M17.5 4.5l2 2L12 14l-3 1 1-3Z",
  music: "M9 17.5V6l10-2v11.5M9 17.5a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0ZM19 15.500a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0Z",
  mic: "M12 4a2.5 2.5 0 0 0-2.5 2.5v5a2.5 2.5 0 0 0 5 0v-5A2.5 2.5 0 0 0 12 4ZM6.5 11a5.5 5.5 0 0 0 11 0M12 16.5V20M9 20h6",
  captions: "M4 6h16v12H4ZM7.5 11h4M13.5 11h3M7.5 14.5h2M11.5 14.5h5",
  cutout: "M12 4a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7ZM5 20a7 7 0 0 1 14 0M3 3l2 2M19 3l2 2",
  video: "M4 6h12v12H4ZM16 10l4-2.5v9L16 14",
  export: "M12 15V4M7.5 8.5 12 4l4.5 4.5M5 14v4a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-4",
  spark: "M12 3l1.9 5.6L19.5 10l-5.6 1.9L12 17.5l-1.9-5.6L4.5 10l5.6-1.4Z",
  media: "M4 5h16v14H4ZM4 15l4.5-4.5 4 4 2.5-2.5 5 5",
  reveal: "M14 5h5v5M19 5l-8 8M11 7H6a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1v-5",
} as const;

export type IconName = keyof typeof PATHS;

/** Decorative by default; the control it sits in carries the label. */
export function Icon({ name, size = 16 }: { name: IconName; size?: number }) {
  return (
    <svg
      className="icon"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
