import { useId } from "react";

interface LogoMarkProps {
  /** True while the assistant or a job is working: the playhead sweeps and the light blinks. */
  working: boolean;
  size?: number;
}

/**
 * The app's mark: Rirote's R with a play cut-out, its leg cut into timeline clips,
 * a playhead and the on-air light. The same drawing as src-tauri/app-icon.svg.
 */
export function LogoMark({ working, size = 28 }: LogoMarkProps) {
  // Gradient and mask ids must be unique in the document.
  const id = useId().replace(/:/g, "");
  return (
    <svg
      className="logo-mark"
      data-working={working}
      viewBox="0 0 1024 1024"
      width={size}
      height={size}
      aria-hidden="true"
    >
      <defs>
        <linearGradient id={`${id}bg`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#13294b" />
          <stop offset="1" stopColor="#0a1628" />
        </linearGradient>
        <linearGradient id={`${id}r`} x1="0" y1="1" x2="1" y2="0">
          <stop offset="0" stopColor="#0077b6" />
          <stop offset="1" stopColor="#00d4ff" />
        </linearGradient>
        <mask id={`${id}slices`} maskUnits="userSpaceOnUse" x="0" y="0" width="64" height="64">
          <rect x="30" y="41.9" width="30" height="3.6" fill="#fff" />
          <rect x="30" y="46.8" width="30" height="3.6" fill="#fff" />
          <rect x="30" y="51.7" width="30" height="3.4" fill="#fff" />
        </mask>
      </defs>
      <rect x="100" y="100" width="824" height="824" rx="186" fill={`url(#${id}bg)`} />
      <g transform="translate(172 188) scale(10)">
        <path
          fill={`url(#${id}r)`}
          fillRule="evenodd"
          d="M13 9H37C46.9 9 54 15.8 54 24.8C54 33.6 47.2 40.6 37 40.6H25V55H13ZM27.6 15.2V33.4L43.4 24.3Z"
        />
        <path fill="#06d6a0" mask={`url(#${id}slices)`} d="M33.4 40.6L44.4 39L54 55H41.6Z" />
      </g>
      <circle className="logo-mark__tally" cx="796" cy="228" r="44" fill="#ff4d5e" />
      <g className="logo-mark__head" fill="#ffd166">
        <rect x="428" y="250" width="22" height="512" rx="11" />
        <path d="M411 218h56v30l-28 26-28-26z" />
      </g>
    </svg>
  );
}
