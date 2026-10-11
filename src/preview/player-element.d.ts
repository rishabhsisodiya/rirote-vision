// Lets JSX use the <hyperframes-player> web component.

import type { HyperframesPlayer } from "@hyperframes/player";
import type { DetailedHTMLProps, HTMLAttributes } from "react";

declare module "react" {
  namespace JSX {
    interface IntrinsicElements {
      "hyperframes-player": DetailedHTMLProps<HTMLAttributes<HyperframesPlayer>, HyperframesPlayer> & {
        src?: string;
        muted?: boolean;
        loop?: boolean;
        "disable-click-to-play"?: boolean;
      };
    }
  }
}
