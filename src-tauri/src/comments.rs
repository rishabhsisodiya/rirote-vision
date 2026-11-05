//! Review notes pinned to a moment and a spot on the frame, kept in the
//! project's `.rirote/comments.json`. "Fix with AI" hands the open ones to the
//! assistant.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::{ids, store};

const MAX_TEXT: usize = 2_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    /// Seconds on the timeline.
    pub time: f64,
    /// Where on the frame, as fractions of its width and height, if pinned.
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub text: String,
    pub resolved: bool,
    /// Seconds since 1970.
    pub created_at: u64,
}

pub struct Comments {
    lock: Mutex<()>,
}

fn file(project: &Path) -> PathBuf {
    project.join(".rirote").join("comments.json")
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |time| time.as_secs())
}

fn check(time: f64, x: Option<f64>, y: Option<f64>, text: &str) -> AppResult<()> {
    if !time.is_finite() || time < 0.0 {
        return Err(AppError::InvalidInput("A note needs a time on the timeline.".into()));
    }
    if [x, y].into_iter().flatten().any(|v| !(0.0..=1.0).contains(&v)) {
        return Err(AppError::InvalidInput("A note's spot must be inside the frame.".into()));
    }
    if text.trim().is_empty() || text.len() > MAX_TEXT {
        return Err(AppError::InvalidInput(format!("Write the note in 1 to {MAX_TEXT} characters.")));
    }
    Ok(())
}

impl Comments {
    pub fn new() -> Self {
        Self { lock: Mutex::new(()) }
    }

    /// In time order.
    pub fn list(&self, project: &Path) -> Vec<Comment> {
        let mut list: Vec<Comment> = store::read_or_default(&file(project));
        list.sort_by(|a, b| a.time.total_cmp(&b.time));
        list
    }

    pub fn add(&self, project: &Path, time: f64, x: Option<f64>, y: Option<f64>, text: &str) -> AppResult<Vec<Comment>> {
        check(time, x, y, text)?;
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut list = self.list(project);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        list.push(Comment {
            id: format!("c-{:012x}", ids::fnv1a(format!("{nanos}{text}").as_bytes()) & 0xffff_ffff_ffff),
            time,
            x,
            y,
            text: text.trim().to_owned(),
            resolved: false,
            created_at: now(),
        });
        store::write_atomic(&file(project), &list)?;
        Ok(self.list(project))
    }

    /// Changes a note's text or marks it resolved (or open again).
    pub fn update(&self, project: &Path, id: &str, text: Option<&str>, resolved: Option<bool>) -> AppResult<Vec<Comment>> {
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut list = self.list(project);
        let comment = list.iter_mut().find(|c| c.id == id).ok_or_else(|| AppError::NotFound("That note is gone.".into()))?;
        if let Some(text) = text {
            check(comment.time, comment.x, comment.y, text)?;
            comment.text = text.trim().to_owned();
        }
        if let Some(resolved) = resolved {
            comment.resolved = resolved;
        }
        store::write_atomic(&file(project), &list)?;
        Ok(self.list(project))
    }

    pub fn delete(&self, project: &Path, id: &str) -> AppResult<Vec<Comment>> {
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut list = self.list(project);
        list.retain(|c| c.id != id);
        store::write_atomic(&file(project), &list)?;
        Ok(self.list(project))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_added_resolved_edited_and_deleted_in_time_order() {
        let project = tempfile::tempdir().unwrap();
        let notes = Comments::new();
        notes.add(project.path(), 8.2, Some(0.7), Some(0.2), "Logo too close to the top").unwrap();
        let list = notes.add(project.path(), 1.5, None, None, " Hook is slow ").unwrap();
        assert_eq!(list.iter().map(|c| c.time).collect::<Vec<_>>(), vec![1.5, 8.2]);
        assert_eq!(list[0].text, "Hook is slow");
        let id = list[1].id.clone();
        let list = notes.update(project.path(), &id, Some("Logo lower"), Some(true)).unwrap();
        assert!(list[1].resolved && list[1].text == "Logo lower");
        assert_eq!(notes.delete(project.path(), &id).unwrap().len(), 1);
    }

    #[test]
    fn bad_notes_are_refused() {
        let project = tempfile::tempdir().unwrap();
        let notes = Comments::new();
        assert!(notes.add(project.path(), -1.0, None, None, "x").is_err());
        assert!(notes.add(project.path(), 1.0, Some(1.5), Some(0.5), "x").is_err());
        assert!(notes.add(project.path(), 1.0, None, None, "   ").is_err());
        assert_eq!(notes.update(project.path(), "c-none", None, Some(true)).unwrap_err().code(), "not_found");
    }
}
