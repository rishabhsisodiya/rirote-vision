import { convertFileSrc } from "@tauri-apps/api/core";

import type { Blueprint } from "../lib/types";

interface BlueprintViewProps {
  blueprint: Blueprint;
  /** The reference's folder, which the stills' paths are relative to. */
  folder: string;
}

const seconds = (value: number) => `${value.toFixed(1)} s`;

/** What the study found: the format laid out the way an editor reads a cut. */
export function BlueprintView({ blueprint, folder }: BlueprintViewProps) {
  const { pacing } = blueprint;
  const total = pacing.totalDuration || 1;
  return (
    <div className="blueprint">
      <p className="blueprint__summary">{blueprint.summary}</p>

      <dl className="blueprint__figures">
        <div>
          <dt>Length</dt>
          <dd className="mono">{seconds(pacing.totalDuration)}</dd>
        </div>
        <div>
          <dt>Shots</dt>
          <dd className="mono">{pacing.shotCount}</dd>
        </div>
        <div>
          <dt>Average shot</dt>
          <dd className="mono">
            {seconds(pacing.avgShotLength)} <small>({seconds(pacing.minShotLength)}–{seconds(pacing.maxShotLength)})</small>
          </dd>
        </div>
        <div>
          <dt>Cuts every 10 s</dt>
          <dd className="mono">{pacing.cutsPer10s.toFixed(1)}</dd>
        </div>
      </dl>

      <section aria-labelledby="bp-structure">
        <h3 id="bp-structure">Structure</h3>
        <ol className="blueprint__strip" aria-label="Sections in time order">
          {blueprint.structure.map((section, index) => (
            <li
              key={index}
              className={`blueprint__section blueprint__section--${section.role}`}
              style={{ flexGrow: Math.max(0.5, section.end - section.start) / total }}
              title={`${seconds(section.start)}–${seconds(section.end)}: ${section.summary}`}
            >
              <span className="blueprint__role">{section.role}</span>
              <span className="mono">{seconds(section.start)}</span>
            </li>
          ))}
        </ol>
        <ul className="blueprint__sections">
          {blueprint.structure.map((section, index) => (
            <li key={index}>
              <span className="mono">
                {seconds(section.start)}–{seconds(section.end)}
              </span>{" "}
              <strong>{section.role}</strong> {section.summary}
            </li>
          ))}
        </ul>
      </section>

      <section aria-labelledby="bp-hook">
        <h3 id="bp-hook">
          Hook · first {blueprint.hook.window_seconds.toFixed(0)} s · <span className="blueprint__tag">{blueprint.hook.hook_type}</span>
        </h3>
        <p>
          <strong>Seen:</strong> {blueprint.hook.visual}
        </p>
        {blueprint.hook.spoken ? (
          <p>
            <strong>Said:</strong> {blueprint.hook.spoken}
          </p>
        ) : null}
        <p className="blueprint__why">{blueprint.hook.analysis}</p>
      </section>

      <section aria-labelledby="bp-shots">
        <h3 id="bp-shots">Shots studied · {blueprint.shots.length}</h3>
        <ol className="blueprint__shots">
          {blueprint.shots.map((shot) => (
            <li key={shot.index} className="shot">
              <span className="shot__still">
                {shot.keyframe ? <img src={convertFileSrc(`${folder}/${shot.keyframe}`)} alt="" loading="lazy" /> : null}
              </span>
              <span className="shot__body">
                <span className="shot__kind">
                  <span className="mono">
                    {seconds(shot.start)}–{seconds(shot.end)}
                  </span>{" "}
                  · {shot.shot_type} · {shot.camera_motion} · <span className={`energy energy--${shot.energy}`}>{shot.energy}</span>
                </span>
                <span className="shot__what">{shot.subject_action}</span>
                {shot.on_screen_text ? <span className="shot__text">Text: {shot.on_screen_text}</span> : null}
              </span>
            </li>
          ))}
        </ol>
      </section>

      <section aria-labelledby="bp-finish" className="blueprint__finish">
        <h3 id="bp-finish">Captions and music</h3>
        <p>
          <strong>Captions:</strong>{" "}
          {blueprint.captionStyle.present
            ? `${blueprint.captionStyle.position} · ${blueprint.captionStyle.style}${blueprint.captionStyle.animation ? ` · ${blueprint.captionStyle.animation}` : ""}`
            : "none"}
        </p>
        <p>
          <strong>Music:</strong> {blueprint.music.mood} · {blueprint.music.tempo} tempo
          {blueprint.music.notes ? <span className="blueprint__note"> ({blueprint.music.notes})</span> : null}
        </p>
        <p className="blueprint__note">
          Studied with {blueprint.model}
          {blueprint.language ? ` · speech in ${blueprint.language}` : ""}.
        </p>
      </section>
    </div>
  );
}
