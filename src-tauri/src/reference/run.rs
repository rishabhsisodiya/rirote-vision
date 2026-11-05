//! Running a study's tools so that Stop ends them, and everything they started.

use std::process::Output;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::process::{Child, Command};
use tokio::sync::Notify;

use crate::error::{AppError, AppResult};

/// Raised once to stop the running study.
#[derive(Clone, Default)]
pub struct Cancel {
    raised: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl Cancel {
    pub fn raise(&self) {
        self.raised.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn raised(&self) -> bool {
        self.raised.load(Ordering::SeqCst)
    }

    pub fn check(&self) -> AppResult<()> {
        if self.raised() {
            Err(stopped())
        } else {
            Ok(())
        }
    }
}

pub fn stopped() -> AppError {
    AppError::Refused("Stopped.".into())
}

/// Runs `command` in its own process group: to the end, until `limit`, or
/// until `cancel` is raised, when the whole group is ended.
pub async fn run(mut command: Command, limit: Duration, what: &str, cancel: &Cancel) -> AppResult<Output> {
    cancel.check()?;
    command.process_group(0);
    let child = command
        .spawn()
        .map_err(|error| AppError::ToolFailed(format!("Could not start {what}: {error}")))?;
    wait(child, limit, what, cancel).await
}

/// Waits for a child started in its own process group (`process_group(0)`).
pub async fn wait(child: Child, limit: Duration, what: &str, cancel: &Cancel) -> AppResult<Output> {
    let group = child.id().map(|pid| pid as i32);
    let end_group = || {
        if let Some(pid) = group {
            // SAFETY: signals a process group this module's caller started.
            unsafe { libc::killpg(pid, libc::SIGTERM) };
        }
    };
    let waiting = cancel.notify.notified();
    tokio::pin!(waiting);
    if cancel.raised() {
        end_group();
        return Err(stopped());
    }
    tokio::select! {
        output = tokio::time::timeout(limit, child.wait_with_output()) => match output {
            Ok(output) => Ok(output?),
            Err(_) => {
                end_group();
                Err(AppError::Timeout(format!("{what} did not finish within {} seconds.", limit.as_secs())))
            }
        },
        () = &mut waiting => {
            end_group();
            Err(stopped())
        }
    }
}
