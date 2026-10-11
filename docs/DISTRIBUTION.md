# Can anyone install and run RIROTE VISION?

**As of 11 October 2026, version 0.1.0: not yet.** It runs on the Mac it was built on because that Mac already has everything it depends on. Someone else can get it working today, but only with manual setup and a macOS security warning to clear.

This note records what stands between the current build and a simple "download and run", and the three realistic ways forward.

## What stops a simple install

| Gap | What happens on another Mac | What fixes it |
|---|---|---|
| **Signing** | The app is signed ad hoc, not by an Apple developer account. macOS refuses to open it or calls it damaged, and the person has to override that by hand. | An Apple Developer account (about $99 a year), then signing and notarising the app |
| **No installer** | There is only the `.app` folder: no disk image and no updates. | Add a `.dmg` build target, and later an updater |
| **Apple Silicon only** | The shared build does not run on an Intel Mac. It needs macOS 13 or later. | A universal build |
| **Claude Code** | Each person needs Claude Code installed and signed in with their own paid Claude plan. The app does not install it. | First-run instructions; the app already detects it |
| **Node.js, FFmpeg, Git** | Needed for the engine, for rendering and for the skills install. Check Setup reports them as missing but cannot install them. | Instructions, or bundling Node and FFmpeg inside the app |
| **Python 3.10 to 3.13** | Needed only for voiceover and music. | Instructions, or bundling Python |

Check Setup does install three things for a new user: the video engine, the video skills, and the voiceover and music packages.

## What is not known

- **It has never run on a clean Mac.** Every test was on one machine. Expect first-run surprises, for example the engine's render browser download, which that machine already had.
- **The macOS warning steps are untested for this build.** The README gives the usual ways to clear the warning (`xattr -dr com.apple.quarantine`, or Open Anyway in System Settings). Neither has been tried with this app on a second Mac.
- **Anthropic's terms, in part.** Anthropic's [Legal and compliance page for Claude Code](https://code.claude.com/docs/en/legal-and-compliance) (read 11 October 2026) forbids third-party apps from offering Claude.ai login, routing requests through Free, Pro or Max plan credentials on users' behalf, or handling Claude credentials. It also says this does not prevent "an end user from signing in to the unmodified Claude Code binary with their own Claude subscription". This app works the second way: it runs the user's own, unmodified Claude Code, never sees a credential, and each person's usage is billed to them. What is still open is the page's condition for "running Claude Code in your products or services": agreeing to Anthropic's Commercial Terms of Service. Whether and how that applies to a free open-source app is a question for Anthropic, and the page gives a sales contact for exactly that. The terms have changed several times in 2026, so re-read the page before any release.

## Three ways forward

### 1. Share with a teammate now

Send the app, the README, and stay available for the first run. This costs nothing to build and suits one or two technical people.

To make the file to send (run after quitting the app):

```bash
ditto -c -k --keepParent "/Applications/RIROTE VISION.app" ~/Desktop/RIROTE-VISION.zip
```

The teammate follows **Install, Option B** in the README. If the macOS warning cannot be cleared, they can use Option A and build from source, which avoids it.

### 2. Make it properly installable

Roughly a few working sessions:

1. Apple Developer account, signing and notarisation.
2. A `.dmg` build target.
3. A universal build for Intel Macs.
4. Bundle Node and FFmpeg inside the app, so Check Setup has less to ask for.
5. A first install on a clean Mac, fixing what it turns up.

### 3. Make it a product

Everything in path 2, plus:

1. Confirmation from Anthropic that the app's way of running Claude Code is acceptable for a product you charge for, and what agreeing to their Commercial Terms involves. Claude Code already lets each user sign in with an API key instead of a plan, so no separate API-key mode is needed in the app.
2. Automatic updates.
3. Licence notices for the bundled parts: HyperFrames (Apache-2.0), the fonts, and the speech and segmentation models the engine downloads.
4. A licence notices file shipped inside the app bundle itself. The repository is MIT-licensed and lists its third-party software in the README, but a distributed build should carry those notices too.

## Things a new user's Mac gets changed

Worth telling anyone you share it with:

- `~/Library/Application Support/com.riroteglobal.vision/` holds the app's engine, voice environment, conversations and logs.
- `~/.claude/skills/` and `~/.agents/skills/` receive HyperFrames' skills, where Claude Code in other apps can use them too.
- `~/.cache/hyperframes/` holds models and a browser the engine downloads.
- Each opened project gets a `.hyperframes/` folder and ids added to its `index.html`.
