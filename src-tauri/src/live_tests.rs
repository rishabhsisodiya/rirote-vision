//! End-to-end checks against the real machine and the npm registry. They
//! download the engine, so they only run when asked for:
//! `cargo test -p rirote-vision -- --ignored --nocapture`

use crate::project::Resolution;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::preview::{PreviewEvent, PreviewManager};
use crate::timeline::{EditOp, TimelineService};
use crate::{doctor, hyperframes, project};

#[tokio::test]
#[ignore = "downloads the video engine and runs it"]
async fn install_check_create_and_read_a_project() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();

    let before = doctor::run(data.path()).await;
    assert!(!before.ready, "a fresh folder has no engine");

    hyperframes::install(data.path()).await.expect("engine installs");
    let after = doctor::run(data.path()).await;
    for check in &after.checks {
        println!("{:?} {} — {}", check.status, check.label, check.detail);
    }
    assert!(after.ready, "this Mac should pass every required check");

    let created = project::create(data.path(), work.path(), "Live Test Film", Resolution::Portrait)
        .await
        .expect("project is created");
    assert_eq!(created.name, "live-test-film");

    let info = project::info(data.path(), std::path::Path::new(&created.path)).await.expect("info");
    println!("{info:?}");
    assert_eq!((info.width, info.height), (1080, 1920));

    let again = project::create(data.path(), work.path(), "Live Test Film", Resolution::Portrait).await;
    assert_eq!(again.unwrap_err().code(), "already_exists");

    // Timeline: read, edit, refuse, undo and redo against the real engine.
    let project_dir = std::path::PathBuf::from(&created.path);
    let timelines = TimelineService::new(data.path());
    let first = timelines.read(data.path(), &project_dir).await.expect("timeline reads");
    println!("{:?}", first.timeline);
    let title = first.timeline.clips.iter().find(|clip| clip.id == "title").expect("the blank project has a title");
    assert!(title.editable && !first.can_undo);
    let (start, end) = (title.start, title.end);
    let find = |state: &crate::timeline::TimelineState, id: &str| {
        state.timeline.clips.iter().find(|clip| clip.id == id).map(|clip| (clip.start, clip.end))
    };
    let op = |op| timelines.edit(data.path(), &project_dir, op);

    let trimmed = op(EditOp::TrimEnd { clip: "#title".into(), end: end - 4.0 }).await.expect("trim end");
    assert_eq!(find(&trimmed, "title"), Some((start, end - 4.0)));
    let moved = op(EditOp::Move { clip: "#title".into(), start: 2.0 }).await.expect("move");
    assert_eq!(find(&moved, "title"), Some((2.0, 2.0 + end - 4.0 - start)));
    let head = op(EditOp::TrimStart { clip: "#title".into(), start: 3.0 }).await.expect("trim start");
    assert_eq!(find(&head, "title").map(|(s, _)| s), Some(3.0));
    assert_eq!(find(&head, "title").map(|(_, e)| e), find(&moved, "title").map(|(_, e)| e), "the end stays");
    let split = op(EditOp::Split { clip: "#title".into(), time: 4.0 }).await.expect("split");
    assert_eq!(split.timeline.clips.len(), head.timeline.clips.len() + 1);
    let lifted = op(EditOp::Place { clip: "#title".into(), start: 3.0, track: 4 }).await.expect("place on another track");
    assert_eq!(lifted.timeline.clips.iter().find(|clip| clip.id == "title").map(|clip| clip.track), Some(4));
    let back = timelines.undo(data.path(), &project_dir).await.expect("undo place");
    assert_eq!(back.timeline, split.timeline);
    let silent = op(EditOp::SetVolume { clip: "#title".into(), volume: 0.5 }).await;
    assert_eq!(silent.unwrap_err().code(), "refused", "a title has no sound");
    let refused = op(EditOp::Split { clip: "#title".into(), time: 99.0 }).await;
    assert_eq!(refused.unwrap_err().code(), "refused");
    let missing = op(EditOp::Delete { clip: "#nope".into() }).await;
    assert_eq!(missing.unwrap_err().code(), "not_found");

    let mut state = timelines.undo(data.path(), &project_dir).await.expect("undo split");
    assert_eq!(state.timeline, head.timeline);
    assert!(state.can_redo);
    for _ in 0..3 {
        state = timelines.undo(data.path(), &project_dir).await.expect("undo");
    }
    assert_eq!(state.timeline, first.timeline, "four undos return to the start");
    assert!(!state.can_undo && state.can_redo);
    state = timelines.redo(data.path(), &project_dir).await.expect("redo the first edit");
    assert_eq!(state.timeline, trimmed.timeline);

    // The engine keeps only ten backups of a file. The app keeps its own
    // copies, so a long run of edits can still be undone all the way back.
    for end in [6.5, 7.0, 7.5, 8.0, 8.5, 9.0, 9.5, 9.0, 8.0, 7.0, 6.0, 5.5] {
        op(EditOp::TrimEnd { clip: "#title".into(), end }).await.expect("trim");
    }
    let mut undone = 0;
    let mut state = timelines.read(data.path(), &project_dir).await.expect("read");
    while state.can_undo {
        state = timelines.undo(data.path(), &project_dir).await.expect("every offered undo works");
        undone += 1;
        assert!(undone < 20);
    }
    println!("{undone} of 13 edits could be undone");
    assert_eq!(undone, 13);
    assert_eq!(state.timeline, first.timeline, "back at the very start");
    for _ in 0..13 {
        state = timelines.redo(data.path(), &project_dir).await.expect("redo");
    }
    assert!(!state.can_redo);
    assert_eq!(state.timeline.clips.iter().find(|clip| clip.id == "title").map(|clip| clip.end), Some(5.5));

    // An edit made behind the timeline's back makes the old receipts useless.
    let beyond = op(EditOp::Move { clip: "#title".into(), start: 9.0 }).await;
    assert_eq!(beyond.unwrap_err().code(), "refused", "a clip may not run past the composition");
    op(EditOp::TrimEnd { clip: "#title".into(), end: 5.0 }).await.expect("trim again");
    let index = project_dir.join("index.html");
    std::fs::write(&index, std::fs::read_to_string(&index).unwrap() + "<!-- outside -->").unwrap();
    let stale = timelines.undo(data.path(), &project_dir).await;
    assert_eq!(stale.unwrap_err().code(), "refused");
    assert!(!timelines.read(data.path(), &project_dir).await.unwrap().can_undo);

    // Preview: starts, serves the composition, notices an edit, stops cleanly.
    let events: Arc<Mutex<Vec<PreviewEvent>>> = Arc::default();
    let sink = events.clone();
    let listener = Arc::new(move |event| sink.lock().unwrap().push(event));
    let previews = PreviewManager::new(data.path());

    let session = previews.start(&project_dir, false, listener.clone()).await.expect("preview starts");
    println!("{session:?}");
    assert!(session.url.starts_with("http://127.0.0.1:"));
    assert!(session.url.ends_with("/api/projects/live-test-film/preview"), "named after its folder");
    assert!(!session.signature.is_empty());
    let second = previews.start(&project_dir, false, listener.clone()).await.expect("second start");
    assert_eq!(second.url, session.url, "a running preview is shared");
    assert_ne!(second.lease, session.lease);
    previews.release(session.lease).await;
    assert!(fetch(&session.url).await.contains("data-composition-id"), "still held by the second");

    let fresh = previews.start(&project_dir, true, listener).await.expect("fresh start");
    assert_ne!(fresh.url, session.url, "a fresh start replaces the server");
    previews.release(second.lease).await;
    let session = fresh;

    let page = fetch(&session.url).await;
    assert!(page.contains("data-composition-id"), "the preview serves the composition");
    let runtime = fetch(&session.url.replace("/projects/live-test-film/preview", "/runtime.js")).await;
    assert!(runtime.contains("Xt=.15,ap=1/16"), "the preview runtime has the looser sync tolerance");
    assert!(!runtime.contains("Xt=.04,ap=1/16"));

    let index = project_dir.join("index.html");
    let edited = std::fs::read_to_string(&index).unwrap().replace("</body>", "<!-- edit --></body>");
    std::fs::write(&index, edited).unwrap();
    tokio::time::sleep(Duration::from_secs(4)).await;
    let seen = events.lock().unwrap().clone();
    println!("{seen:?}");
    assert!(
        seen.iter().any(|event| matches!(event, PreviewEvent::Changed { signature, .. } if *signature != session.signature)),
        "an edit on disk is reported"
    );

    previews.release(session.lease).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let port: u16 = session.url.split(':').nth(2).unwrap().split('/').next().unwrap().parse().unwrap();
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_err(),
        "the server is gone after stop"
    );
}

async fn fetch(url: &str) -> String {
    let rest = url.strip_prefix("http://").unwrap();
    let (host, path) = rest.split_once('/').unwrap();
    let mut stream = tokio::net::TcpStream::connect(host).await.unwrap();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    stream.write_all(format!("GET /{path} HTTP/1.0\r\nHost: {host}\r\n\r\n").as_bytes()).await.unwrap();
    let mut body = Vec::new();
    stream.read_to_end(&mut body).await.unwrap();
    String::from_utf8_lossy(&body).into_owned()
}

/// Two short turns through the real Claude Code, on the user's own login.
/// Run alone: `cargo test -p rirote-vision assistant -- --ignored --nocapture`
#[tokio::test]
#[ignore = "runs Claude Code and uses the signed-in plan"]
async fn assistant_writes_in_the_project_and_asks_before_a_command() {
    use crate::agent::{AgentEvent, AgentService, Answer, ClaudeCode, EditorContext};
    use std::path::Path;

    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let project = work.path().canonicalize().unwrap();
    let events: Arc<Mutex<Vec<AgentEvent>>> = Arc::default();
    let sink = events.clone();
    let listener = Arc::new(move |_: &Path, event: AgentEvent| sink.lock().unwrap().push(event));
    let agent = AgentService::new(data.path(), Arc::new(ClaudeCode { model: Some("haiku".into()) }));

    let wait_for = |what: fn(&AgentEvent) -> bool| {
        let events = events.clone();
        async move {
            for _ in 0..600 {
                if let Some(found) = events.lock().unwrap().iter().find(|event| what(event)).cloned() {
                    return found;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            panic!("timed out; saw {:?}", events.lock().unwrap());
        }
    };
    let ended = |event: &AgentEvent| {
        matches!(event, AgentEvent::Finished { .. } | AgentEvent::Failed { .. } | AgentEvent::Stopped)
    };

    let unset = agent.send(&project, "hello", &EditorContext::default(), listener.clone()).await;
    assert_eq!(unset.unwrap_err().code(), "refused", "a level must be chosen first");
    agent.set_trust(&project, crate::agent::Trust::Ask).expect("level is saved");

    // A file inside the project is written without asking.
    let prompt = "Use the Write tool to create a file named note.txt in the current folder containing exactly: ok. Then reply with one short sentence.";
    agent.send(&project, prompt, &EditorContext::default(), listener.clone()).await.expect("turn starts");
    assert!(agent.view(&project).busy);
    let busy = agent.send(&project, "and another", &EditorContext::default(), listener.clone()).await;
    assert_eq!(busy.unwrap_err().code(), "refused", "one turn at a time");
    let end = wait_for(ended).await;
    println!("first turn: {:?}", events.lock().unwrap().iter().filter(|e| !matches!(e, AgentEvent::TextDelta { .. })).collect::<Vec<_>>());
    assert!(matches!(end, AgentEvent::Finished { ok: true, .. }), "{end:?}");
    assert_eq!(std::fs::read_to_string(project.join("note.txt")).unwrap().trim(), "ok");
    assert!(!events.lock().unwrap().iter().any(|event| matches!(event, AgentEvent::Approval { .. })));
    assert!(events.lock().unwrap().iter().any(|event| matches!(event, AgentEvent::TextDelta { .. })), "the reply streams");
    tokio::time::sleep(Duration::from_millis(500)).await;
    let view = agent.view(&project);
    assert!(!view.busy);
    assert_eq!(view.messages[0].role, "user");
    assert!(view.messages.iter().any(|m| m.role == "tool" && m.text == "note.txt" && m.ok == Some(true)), "{:?}", view.messages);

    // A shell command waits for an answer, and the conversation continues.
    events.lock().unwrap().clear();
    let prompt = "Use the Bash tool to run exactly this command: touch approved.txt  Then tell me in one sentence which file you created in your previous turn.";
    agent.send(&project, prompt, &EditorContext::default(), listener).await.expect("second turn starts");
    let asked = wait_for(|event| matches!(event, AgentEvent::Approval { .. } | AgentEvent::Finished { .. } | AgentEvent::Failed { .. })).await;
    let AgentEvent::Approval { request_id, tool, detail, .. } = asked else { panic!("expected a request, got {asked:?}") };
    println!("asked: {tool} {detail}");
    assert_eq!(tool, "Bash");
    assert!(!project.join("approved.txt").exists(), "nothing runs before the answer");
    agent.answer(&request_id, Answer::Allow).await.expect("answer is passed on");
    let end = wait_for(ended).await;
    let AgentEvent::Finished { ok: true, text } = end else { panic!("{end:?}") };
    println!("second turn said: {text}");
    assert!(project.join("approved.txt").exists());
    assert!(text.contains("note.txt"), "it remembers the first turn");
}

/// The whole loop: a plain-language request changes a real composition.
/// Run alone: `cargo test -p rirote-vision assistant_edits -- --ignored --nocapture`
#[tokio::test]
#[ignore = "downloads the engine, runs Claude Code and uses the signed-in plan"]
async fn assistant_edits_a_real_project_from_a_plain_request() {
    use crate::agent::{AgentEvent, AgentService, Answer, ClaudeCode, EditorContext};
    use std::path::Path;

    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    hyperframes::install(data.path()).await.expect("engine installs");
    let created = project::create(data.path(), work.path(), "Agent Film", Resolution::Landscape)
        .await
        .expect("project is created");
    let project_dir = std::path::PathBuf::from(&created.path);

    let events: Arc<Mutex<Vec<AgentEvent>>> = Arc::default();
    let sink = events.clone();
    let listener = Arc::new(move |_: &Path, event: AgentEvent| sink.lock().unwrap().push(event));
    let agent = AgentService::new(data.path(), Arc::new(ClaudeCode { model: Some("haiku".into()) }));
    agent.set_trust(&project_dir, crate::agent::Trust::Ask).expect("level is saved");
    let context: EditorContext =
        serde_json::from_value(serde_json::json!({ "playhead": 5.0, "selected": { "ref": "#title", "start": 0.0, "end": 10.0 } })).unwrap();
    agent.send(&project_dir, "Make this clip end here.", &context, listener).await.expect("turn starts");

    // Answer every request with Allow, as a user watching would.
    let mut answered = 0;
    let end = loop {
        tokio::time::sleep(Duration::from_millis(300)).await;
        let seen = events.lock().unwrap().clone();
        for event in seen.iter().skip(answered) {
            if let AgentEvent::Approval { request_id, tool, detail, rule } = event {
                println!("asked: {tool} | {detail} | rule {rule:?}");
                agent.answer(request_id, Answer::Allow).await.expect("answer");
            }
        }
        answered = seen.len();
        if let Some(end) = seen.iter().find(|e| matches!(e, AgentEvent::Finished { .. } | AgentEvent::Failed { .. })) {
            break end.clone();
        }
    };
    for event in events.lock().unwrap().iter() {
        match event {
            AgentEvent::Tool { tool, detail, .. } => println!("tool: {tool} | {detail}"),
            AgentEvent::Text { text } => println!("said: {text}"),
            _ => {}
        }
    }
    assert!(matches!(end, AgentEvent::Finished { ok: true, .. }), "{end:?}");
    let timeline = TimelineService::new(data.path()).read(data.path(), &project_dir).await.expect("timeline reads");
    let title = timeline.timeline.clips.iter().find(|clip| clip.id == "title").expect("title is still there");
    println!("title now {}–{}", title.start, title.end);
    assert!((title.end - 5.0).abs() < 0.01, "the clip ends at the playhead");
}

/// Voiceover, captions and a cut-out made for real, through the queue.
/// Run alone: `cargo test -p rirote-vision media_jobs -- --ignored --nocapture`
#[tokio::test]
#[ignore = "downloads the engine, the voice packages and a segmentation model"]
async fn media_jobs_make_real_files() {
    use crate::jobs::{JobQueue, JobSpec, JobStatus};
    use crate::media::{MediaKind, MediaLibrary};
    use crate::voice;

    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    hyperframes::install(data.path()).await.expect("engine installs");
    let started = std::time::Instant::now();
    voice::install(data.path()).await.expect("voice runtime installs");
    println!("voice runtime installed in {:?}", started.elapsed());
    assert!(voice::python(data.path()).is_some());
    let report = doctor::run(data.path()).await;
    let kokoro = report.checks.iter().find(|check| check.id == "kokoro").expect("kokoro is checked");
    println!("{:?} {}", kokoro.status, kokoro.detail);
    assert_eq!(kokoro.status, doctor::Status::Ok);

    let created = project::create(data.path(), work.path(), "Media Film", Resolution::Landscape).await.expect("project");
    let project_dir = std::path::PathBuf::from(&created.path);
    let library = MediaLibrary::new();
    let picture = std::path::PathBuf::from("/Users/rishabhsisodiya/startup/rirote-global/assets/brand/avatar-dark-1024.png");
    let imported = library.import(&project_dir, &[picture]).await.expect("import");
    assert_eq!(imported[0].path, "assets/avatar-dark-1024.png");

    let queue = Arc::new(JobQueue::new(data.path()));
    tokio::spawn(queue.clone().work());
    let finish = |id: u64| {
        let (queue, project_dir) = (queue.clone(), project_dir.clone());
        async move {
            for _ in 0..3000 {
                let job = queue.list(&project_dir).into_iter().find(|job| job.id == id).unwrap();
                if !matches!(job.status, JobStatus::Queued | JobStatus::Running) {
                    return job;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            panic!("job {id} did not finish");
        }
    };

    // Queue all three at once: they must run in order, one at a time.
    let voiceover = queue.add(&project_dir, JobSpec::Voiceover { text: "Rirote Vision turns a sentence into a finished video.".into(), voice: "af_heart".into() }).unwrap();
    let cutout = queue.add(&project_dir, JobSpec::RemoveBackground { source: "assets/avatar-dark-1024.png".into() }).unwrap();
    let spoken = finish(voiceover.id).await;
    println!("{spoken:?}");
    assert_eq!(spoken.status, JobStatus::Done, "{:?}", spoken.error);
    let wav = spoken.output.clone().unwrap();
    assert!(wav.starts_with("assets/voiceover/rirote-vision-turns") && wav.ends_with(".wav"));

    let captions = queue.add(&project_dir, JobSpec::Transcribe { source: wav.clone() }).unwrap();
    let cut = finish(cutout.id).await;
    println!("{cut:?}");
    let written = finish(captions.id).await;
    println!("{written:?}");
    assert_eq!(written.status, JobStatus::Done, "{:?}", written.error);
    let srt = std::fs::read_to_string(project_dir.join(written.output.as_ref().unwrap())).unwrap();
    println!("{srt}");
    assert!(srt.to_lowercase().contains("video"));
    assert!(project_dir.join(wav.replace(".wav", ".transcript.json")).is_file(), "the transcript is beside the sound");
    assert!(!project_dir.join("transcript.json").exists());
    assert_eq!(cut.status, JobStatus::Done, "{:?}", cut.error);
    assert!(project_dir.join("assets/avatar-dark-1024-cutout.png").is_file());

    // Music beds, from the script inside the app, in the same environment.
    let bed = |style: &str, seconds: f64, impact_at: Option<f64>| JobSpec::Music { style: style.into(), seconds, impact_at };
    let steady = queue.add(&project_dir, bed("steady", 8.0, None)).unwrap();
    let build = queue.add(&project_dir, bed("cinematic", 12.0, Some(7.0))).unwrap();
    let (steady, build) = (finish(steady.id).await, finish(build.id).await);
    println!("{steady:?}\n{build:?}");
    assert_eq!(steady.status, JobStatus::Done, "{:?}", steady.error);
    assert_eq!(steady.output.as_deref(), Some("assets/music/steady-8s.wav"));
    assert_eq!(build.status, JobStatus::Done, "{:?}", build.error);
    let refused = queue.add(&project_dir, bed("cinematic", 12.0, Some(11.5)));
    assert_eq!(refused.unwrap_err().code(), "invalid_input");

    let items = library.list(&project_dir).await.unwrap();
    for item in &items {
        println!("{:?} {} {:?}", item.kind, item.path, item.duration);
    }
    let sound = items.iter().find(|item| item.path.contains("voiceover/")).unwrap();
    assert!(sound.duration.unwrap() > 2.0, "ffprobe measured the voiceover");
    let length = |path: &str| items.iter().find(|item| item.path == path).and_then(|item| item.duration).unwrap();
    assert!((length("assets/music/steady-8s.wav") - 8.0).abs() < 0.05, "the bed is as long as asked");
    assert!((length("assets/music/cinematic-12s.wav") - 12.0).abs() < 0.05);
    assert_eq!(sound.kind, MediaKind::Audio);
    assert_eq!(items.iter().filter(|item| item.kind == MediaKind::Image).count(), 2);
    assert!(items.iter().any(|item| item.kind == MediaKind::Captions));
    assert!(!data.path().join("tmp").read_dir().map(|mut dir| dir.next().is_some()).unwrap_or(false), "scratch is cleaned up");
}

/// A real render through the queue: checked first, followed by percentage,
/// looked at afterwards, and cancellable.
/// Run alone: `cargo test -p rirote-vision export_renders -- --ignored --nocapture`
#[tokio::test]
#[ignore = "downloads the engine and renders video"]
async fn export_renders_a_real_film() {
    use crate::jobs::{Job, JobQueue, JobSpec, JobStatus};
    use crate::render;

    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    hyperframes::install(data.path()).await.expect("engine installs");
    let created = project::create(data.path(), work.path(), "Export Film", Resolution::Portrait).await.expect("project");
    let project_dir = std::path::PathBuf::from(&created.path);

    // The preview tuning must leave the export's own runtime intact.
    assert_eq!(hyperframes::tune_preview(data.path()).expect("tuning runs"), 1);
    let checked = render::check(data.path(), &project_dir).await.expect("checks run");
    println!("{checked:?}");
    assert!(checked.ok, "a blank project has nothing wrong with it");

    let queue = Arc::new(JobQueue::new(data.path()));
    let seen: Arc<Mutex<Vec<Job>>> = Arc::default();
    let sink = seen.clone();
    queue.listen(Arc::new(move |jobs: Vec<Job>| sink.lock().unwrap().extend(jobs)));
    tokio::spawn(queue.clone().work());
    let export = |name: &str, quality: &str| -> JobSpec {
        serde_json::from_value(serde_json::json!({
            "type": "export", "format": "mp4", "quality": quality, "fps": null, "resolution": null, "name": name, "folder": null
        }))
        .unwrap()
    };
    let state_of = |id: u64| queue.list(&project_dir).into_iter().find(|job| job.id == id).unwrap();

    let first = queue.add(&project_dir, export("Draft cut", "draft")).unwrap();
    let second = queue.add(&project_dir, export("To cancel", "standard")).unwrap();
    while matches!(state_of(first.id).status, JobStatus::Queued | JobStatus::Running) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let done = state_of(first.id);
    println!("{done:?}");
    assert_eq!(done.status, JobStatus::Done, "{:?}", done.error);
    assert_eq!(done.output.as_deref(), Some("renders/Draft cut.mp4"));
    let note = done.note.clone().unwrap();
    assert!(note.starts_with("1080 × 1920 · 10.0 s"), "{note}");
    assert!(!note.contains("check the end"), "the film is as long as the composition");
    let stages: Vec<(Option<u8>, String)> = seen.lock().unwrap().iter()
        .filter(|job| job.id == first.id && job.status == JobStatus::Running)
        .filter_map(|job| job.progress.clone().map(|said| (job.percent, said)))
        .collect();
    let percents: Vec<u8> = stages.iter().filter_map(|(percent, _)| *percent).collect();
    println!("saw {} progress updates, e.g. {:?}", stages.len(), stages.iter().step_by(stages.len().max(8) / 8).collect::<Vec<_>>());
    assert!(percents.len() > 5 && percents.windows(2).all(|pair| pair[0] <= pair[1]), "the percentage only goes up");
    assert!(stages.iter().any(|(_, said)| said.starts_with("Capturing frame")));

    // The second export is cancelled once it is under way, and leaves no file.
    while state_of(second.id).percent.unwrap_or(0) < 25 {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    queue.cancel(second.id);
    while matches!(state_of(second.id).status, JobStatus::Queued | JobStatus::Running) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(state_of(second.id).status, JobStatus::Cancelled);
    assert!(!project_dir.join("renders/To cancel.mp4").exists());
    assert!(project_dir.join("renders/Draft cut.mp4").is_file());
}

/// The skills installer, run against a throwaway home folder so the real
/// Claude Code setup is not touched.
/// Run alone: `cargo test -p rirote-vision skills_install -- --ignored --nocapture`
#[tokio::test]
#[ignore = "downloads the engine and HyperFrames' skills from GitHub"]
async fn skills_install_into_an_empty_home() {
    use crate::{shell_env, skills};

    // Tools are found through the real login shell before HOME is changed.
    shell_env::search_path();
    let data = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    hyperframes::install(data.path()).await.expect("engine installs");
    // SAFETY: this test runs alone (it is ignored by default and named on
    // the command line), so nothing else reads the environment meanwhile.
    unsafe {
        std::env::set_var("HOME", home.path());
        std::env::remove_var("CLAUDE_CONFIG_DIR");
    }

    let folder = skills::folder();
    assert_eq!(folder, home.path().join(".claude/skills"));
    assert_eq!(skills::missing(&folder).len(), skills::core_count());
    let before = doctor::run(data.path()).await;
    let row = before.checks.iter().find(|check| check.id == "skills").unwrap();
    assert_eq!((row.status, row.action), (doctor::Status::Missing, Some("install-skills")));
    assert!(!before.ready);

    let started = std::time::Instant::now();
    skills::install(data.path()).await.expect("skills install");
    println!("installed in {:?}: {:?}", started.elapsed(), std::fs::read_dir(&folder).unwrap().count());
    assert!(skills::missing(&folder).is_empty());
    assert!(skills::require(&folder).is_ok());
    let after = doctor::run(data.path()).await;
    let row = after.checks.iter().find(|check| check.id == "skills").unwrap();
    println!("{:?} {}", row.status, row.detail);
    assert_eq!((row.status, row.action), (doctor::Status::Ok, None));
}

/// The boundary of each trust level, with the real Claude Code deciding.
/// Run alone: `cargo test -p rirote-vision trust_levels -- --ignored --nocapture`
#[tokio::test]
#[ignore = "downloads the engine, runs Claude Code and uses the signed-in plan"]
async fn trust_levels_ask_only_for_what_they_should() {
    use crate::agent::{AgentEvent, AgentService, Answer, ClaudeCode, EditorContext, Trust};
    use std::path::Path;

    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    hyperframes::install(data.path()).await.expect("engine installs");
    // A folder name with a space, as real project folders have.
    let created = project::create(data.path(), work.path(), "Trust Film", Resolution::Landscape).await.expect("project");
    let project_dir = std::path::PathBuf::from(&created.path);
    let outside = project_dir.parent().unwrap().to_path_buf();
    std::fs::write(project_dir.join("package.json"), r#"{"name":"t","scripts":{"hello":"echo hello-from-script"}}"#).unwrap();

    // Runs one request at a level, denying whatever is asked, and returns
    // the commands that were asked about.
    async fn run(data: &Path, project: &Path, trust: Trust, prompt: String) -> Vec<String> {
        let events: Arc<Mutex<Vec<AgentEvent>>> = Arc::default();
        let sink = events.clone();
        let listener = Arc::new(move |_: &Path, event: AgentEvent| sink.lock().unwrap().push(event));
        let agent = AgentService::new(data, Arc::new(ClaudeCode { model: Some("haiku".into()) }));
        agent.set_trust(project, trust).unwrap();
        agent.clear(project).unwrap();
        agent.send(project, &prompt, &EditorContext::default(), listener).await.expect("turn starts");
        let mut answered = 0;
        let mut asked = Vec::new();
        for _ in 0..900 {
            tokio::time::sleep(Duration::from_millis(200)).await;
            let seen = events.lock().unwrap().clone();
            for event in seen.iter().skip(answered) {
                if let AgentEvent::Approval { request_id, detail, .. } = event {
                    asked.push(detail.clone());
                    agent.answer(request_id, Answer::Deny).await.unwrap();
                }
            }
            answered = seen.len();
            if seen.iter().any(|e| matches!(e, AgentEvent::Finished { .. } | AgentEvent::Failed { .. })) {
                for event in seen.iter().filter(|e| !matches!(e, AgentEvent::TextDelta { .. })) {
                    println!("  {trust:?}: {event:?}");
                }
                return asked;
            }
        }
        panic!("the turn did not finish");
    }

    let commands = format!(
        "Run these Bash commands exactly as written, one tool call each, in this order, and do not skip any even if one is denied: \
(1) hyperframes --version (2) cd \"{}\" && npm run hello (3) mkdir -p assets/shots (4) cp package.json assets/shots/copy.json \
(5) ffprobe -version (6) curl -sI https://example.com (7) touch ../outside-the-project.txt (8) hyperframes --version && touch ../chained.txt. \
Then reply with the single word done.",
        project_dir.display()
    );

    let asked = run(data.path(), &project_dir, Trust::Trusted, commands.clone()).await;
    println!("trusted asked about: {asked:#?}");
    assert!(project_dir.join("assets/shots/copy.json").is_file(), "file work in the project ran unasked");
    assert!(!outside.join("outside-the-project.txt").exists() && !outside.join("chained.txt").exists());
    assert_eq!(asked.len(), 3, "only the download, the write outside and the chain that leaves the project ask");
    assert!(asked[0].starts_with("curl "));
    assert!(asked[1].contains("outside-the-project.txt"));
    assert!(asked[2].contains("chained.txt"));

    std::fs::remove_dir_all(project_dir.join("assets/shots")).unwrap();
    let asked = run(data.path(), &project_dir, Trust::Ask, commands.clone()).await;
    println!("ask-every-time asked about {} commands", asked.len());
    assert!(asked.len() >= 6, "every command that changes or reaches out asks: {asked:#?}");
    assert!(!project_dir.join("assets/shots").exists(), "nothing ran that was denied");

    // At full trust the same download runs without a question. (Writing
    // outside the project is not used here: the assistant itself declines
    // that, because the app's briefing tells it to stay inside.)
    let request = "Run this Bash command exactly: curl -sI https://example.com  Then reply with the single word done.";
    let asked = run(data.path(), &project_dir, Trust::Full, request.into()).await;
    assert!(asked.is_empty(), "full trust asks nothing: {asked:#?}");
    let asked = run(data.path(), &project_dir, Trust::Trusted, request.into()).await;
    assert_eq!(asked.len(), 1, "the same request asks at the trusted level: {asked:#?}");
}
