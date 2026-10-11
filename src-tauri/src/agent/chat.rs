//! A project's conversation with the assistant, kept on disk so it is there
//! again when the project is reopened.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::Rule;
use crate::error::{AppError, AppResult};

/// Older messages are dropped from the saved conversation beyond this many.
const MAX_MESSAGES: usize = 400;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    /// user, assistant, tool or notice.
    pub role: String,
    pub text: String,
    /// For a tool message: the tool's name and the id of its call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// For a tool message: whether it worked, once that is known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ok: Option<bool>,
}

impl ChatMessage {
    pub fn said(role: &str, text: &str) -> Self {
        Self { role: role.into(), text: text.into(), tool: None, id: None, ok: None }
    }

    pub fn tool(id: &str, tool: &str, detail: &str) -> Self {
        Self { role: "tool".into(), text: detail.into(), tool: Some(tool.into()), id: Some(id.into()), ok: None }
    }
}

/// How much the assistant may do in a project without asking.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Trust {
    /// Edits files in the project; asks before every command.
    Ask,
    /// Also manages files in the project and runs the video tools unasked.
    Trusted,
    /// Is never asked anything.
    Full,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    #[serde(default)]
    pub project_path: String,
    /// The agent's id for this conversation, used to continue it.
    #[serde(default)]
    pub session: Option<String>,
    /// Standing permissions the user gave in this project.
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// The level the user chose for this project; none until they choose.
    #[serde(default)]
    pub trust: Option<Trust>,
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
}

/// What the window is shown of a conversation.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatView {
    pub messages: Vec<ChatMessage>,
    /// A turn is running for this project right now.
    pub busy: bool,
    pub rules: Vec<Rule>,
    /// None until the user has chosen a level for this project.
    pub trust: Option<Trust>,
}

impl Chat {
    pub fn push(&mut self, message: ChatMessage) {
        self.messages.push(message);
        if self.messages.len() > MAX_MESSAGES {
            let extra = self.messages.len() - MAX_MESSAGES;
            self.messages.drain(..extra);
        }
    }

    pub fn finish_tool(&mut self, id: &str, ok: bool) {
        let call = self.messages.iter_mut().rev().find(|message| message.id.as_deref() == Some(id));
        if let Some(call) = call {
            call.ok = Some(ok);
        }
    }

    /// Tool calls left without a result belong to a turn that ended early.
    pub fn close_open_tools(&mut self) {
        for message in &mut self.messages {
            if message.role == "tool" && message.ok.is_none() {
                message.ok = Some(false);
            }
        }
    }
}

pub struct ChatStore {
    dir: PathBuf,
}

impl ChatStore {
    pub fn new(data_dir: &Path) -> Self {
        Self { dir: data_dir.join("chats") }
    }

    /// One file per project folder, named by a hash of the folder's path.
    fn file(&self, project: &Path) -> PathBuf {
        self.dir.join(format!("{}.json", crate::ids::project_key(project)))
    }

    /// A missing or damaged file is an empty conversation.
    pub fn load(&self, project: &Path) -> Chat {
        let mut chat: Chat = std::fs::read_to_string(self.file(project))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        chat.project_path = project.to_string_lossy().into_owned();
        chat
    }

    pub fn save(&self, project: &Path, chat: &Chat) -> AppResult<()> {
        std::fs::create_dir_all(&self.dir)?;
        let text = serde_json::to_string_pretty(chat).map_err(|error| AppError::Io(error.to_string()))?;
        let file = self.file(project);
        let draft = file.with_extension("json.tmp");
        std::fs::write(&draft, text)?;
        std::fs::rename(&draft, &file)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_conversation_survives_a_save_and_belongs_to_its_project() {
        let dir = tempfile::tempdir().unwrap();
        let store = ChatStore::new(dir.path());
        let (film, other) = (Path::new("/films/a"), Path::new("/films/b"));

        let mut chat = store.load(film);
        assert!(chat.messages.is_empty() && chat.session.is_none());
        chat.session = Some("s1".into());
        chat.trust = Some(Trust::Trusted);
        chat.rules.push(Rule { tool: "Bash".into(), content: "ls".into() });
        chat.push(ChatMessage::said("user", "trim the intro"));
        chat.push(ChatMessage::tool("t1", "Edit", "index.html"));
        chat.finish_tool("t1", true);
        store.save(film, &chat).unwrap();

        let again = store.load(film);
        assert_eq!(again.session.as_deref(), Some("s1"));
        assert_eq!(again.trust, Some(Trust::Trusted));
        assert_eq!(store.load(other).trust, None, "a project starts with no level chosen");
        assert_eq!(again.rules.len(), 1);
        assert_eq!(again.messages[1].ok, Some(true));
        assert!(store.load(other).messages.is_empty());
    }

    #[test]
    fn old_messages_fall_off_and_open_tools_are_closed() {
        let mut chat = Chat::default();
        for index in 0..MAX_MESSAGES + 10 {
            chat.push(ChatMessage::said("user", &index.to_string()));
        }
        assert_eq!(chat.messages.len(), MAX_MESSAGES);
        assert_eq!(chat.messages[0].text, "10");

        chat.push(ChatMessage::tool("t9", "Bash", "ls"));
        chat.close_open_tools();
        assert_eq!(chat.messages.last().unwrap().ok, Some(false));
    }

    #[test]
    fn a_damaged_file_is_an_empty_conversation() {
        let dir = tempfile::tempdir().unwrap();
        let store = ChatStore::new(dir.path());
        std::fs::create_dir_all(dir.path().join("chats")).unwrap();
        std::fs::write(store.file(Path::new("/x")), "{oops").unwrap();
        assert!(store.load(Path::new("/x")).messages.is_empty());
    }

}
