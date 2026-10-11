# RIROTE VISION

An AI video studio for macOS. Describe the video in chat, refine it on a timeline, and export it.

It is built on [HyperFrames](https://github.com/heygen-com/hyperframes), which renders video from HTML compositions, and it uses the Claude Code you already have signed in. The app stores no API key.

**Status:** version 0.1.0, an internal tool. It has only been run on one Apple Silicon Mac (macOS 27). Expect rough edges on a fresh machine, and please report what you hit.

## What it does

- **Assistant:** ask for an edit or a whole video in plain language. It works on the open project and asks before running anything outside the level of trust you give it.
- **Preview and timeline:** play the composition, drag clips to move them, drag edges to trim, split, duplicate, delete, change tracks and volume, with undo and redo.
- **Media:** import footage, images and sound; make voiceover, music beds, captions and background cut-outs on your Mac.
- **Export:** MP4, MOV, WebM or GIF, with checks before and verification after.

## What you need

| Requirement | Why | How to get it |
|---|---|---|
| A Mac with Apple Silicon, macOS 13 or later | The shared build is Apple Silicon only | To use an Intel Mac, build from source |
| [Claude Code](https://claude.com/claude-code), signed in with your own paid Claude plan | The assistant runs on it | Install it, run `claude` once in Terminal and sign in |
| Node.js 22 or later | Runs the video engine | `brew install node` |
| FFmpeg | Encodes exports and reads media | `brew install ffmpeg` |
| Git | Fetches the video skills | `xcode-select --install` |
| Python 3.10 to 3.13 (optional) | Voiceover and music only | `brew install python@3.12` |

The `brew` commands need [Homebrew](https://brew.sh).

## Install

### Option A: the app you were sent

1. Unzip the file and drag **RIROTE VISION** into your Applications folder.
2. macOS will refuse to open it, because the app is not signed by an Apple developer account. Clear that once, in Terminal:

   ```bash
   xattr -dr com.apple.quarantine "/Applications/RIROTE VISION.app"
   ```

3. Open the app.

If you would rather not run that command, try opening the app once, then go to **System Settings > Privacy & Security** and click **Open Anyway**.

### Option B: build it yourself

This avoids the macOS warning and works on Intel Macs. You also need [Rust](https://rustup.rs).

```bash
git clone <this repository>
cd rirote-vision
npm install
npm run install-app
```

`install-app` builds the app and copies it to `/Applications`. The first build takes a few minutes.

## First run

1. Open the app, then choose **RIROTE VISION > Check Setup…** from the menu bar at the top of the screen.
2. Install what it offers, in this order:
   - **Install Video Engine** (required, about 120 MB)
   - **Install Video Skills** (required; these are HyperFrames' instructions for Claude Code, saved in `~/.claude/skills`)
   - **Install Voiceover and Music** (optional)
3. Every required row should say **Ready**. If Claude Code, Node.js or FFmpeg shows as missing, install it from the table above and click **Check Again**.
4. **Render browser** may not be ready on a new Mac. The engine normally downloads its own copy of Chrome when it first needs one, but that has not been tried on a fresh machine. If an export fails for this reason, send the logs (see below).

## Make a video

1. Click **New Project**, give it a name and pick a shape (landscape, portrait or square).
2. The first time you use the assistant in a project, choose how much it may do without asking. **Trusted video work** is the usual choice.
3. Describe what you want, for example:

   > Make a 25-second launch video for https://example.com. Capture the site and use real screenshots. Five scenes: a hook, what we build, two highlights and a closing card with the URL.

4. Approve what it asks, and watch the preview and timeline fill in. Refine by chat or by hand.
5. Use the mic and music-note buttons in the Media panel for narration and a music bed, then **Add to Timeline**.
6. Click **Export**. Files are saved in the project's `renders` folder.

Press **⌘/** in the app for the full list of keyboard shortcuts.

## Good to know

- **Your projects are ordinary folders.** The app opens them in place and never uploads them.
- **Opening a project changes its `index.html`.** The engine adds ids it needs for editing and creates a `.hyperframes` folder.
- **The assistant uses your Claude plan.** Each request counts against your usage limits.
- **Everything else is local.** Voiceover, music, captions and cut-outs run on your Mac. There is no image or song generator.
- **The preview is close, the export is exact.** The preview runs in the app's WebKit window and can differ slightly in audio timing. The exported file is rendered in Chrome.

## If something goes wrong

| Problem | Try |
|---|---|
| "The app is damaged" or it will not open | Run the `xattr` command in Install, Option A |
| A required row in Check Setup stays Missing | Install it, then quit and reopen the app so it sees the new tool |
| The assistant says it needs video skills | Check Setup, then Install Video Skills |
| Voiceover or music fails | Check Setup, then Install Voiceover and Music; it needs Python 3.10 to 3.13 |
| The preview does not start | Click **Start Preview Again**; if it persists, send the logs below |

Logs are in `~/Library/Application Support/com.riroteglobal.vision/logs/`. Send `app.log` and `preview-server.log` with any bug report.

## Developing

```bash
npm install
npm run tauri dev      # run the app with live reload
npm test               # frontend unit tests
cargo test -p rirote-vision                             # Rust unit tests
cargo test -p rirote-vision -- --ignored --nocapture    # live tests
npm run app            # build the app bundle
npm run install-app    # build and copy to /Applications (quit the app first)
```

The live tests download the engine, render video and run real Claude Code sessions on your plan, so they are not run by default.

| Folder | What is in it |
|---|---|
| `src/` | The interface: React and TypeScript |
| `src-tauri/src/` | The app's core in Rust: projects, preview, timeline, assistant, media, export |
| `src-tauri/resources/` | The music synth script carried inside the app |
| `docs/` | Notes on distribution |

The video engine is pinned to HyperFrames 0.8.146 in `src-tauri/src/hyperframes.rs`.
