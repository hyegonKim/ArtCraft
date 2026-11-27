//! Team file list of the collaboration server configured in Preferences.

use figcraft_collab::message::{Access, Admin, AdminReply};
use serde_json::{Value, json};

use crate::live::{DEFAULT_SERVER, NetReply, NetRequest};
use crate::online::{clean_name, same_server, server_host};
use crate::{Result, Session};

/// One shared file listed by a team collaboration server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamFile {
    pub file: String,
    pub name: String,
    pub updated: u64,
    pub people: u32,
    pub access: Access,
}

/// Answer from the Preferences server for its team list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamList {
    /// The server keeps no team list (or is older than team lists).
    None,
    /// Shared files currently listed on the server, and optional web app base URL.
    Files { web: String, files: Vec<TeamFile> },
    /// The server could not be reached.
    Offline(String),
}

/// Team list state for a server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Team {
    pub server: String,
    pub list: TeamList,
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Invite link (`figcraft://join?server=...&file=...&team=1`) that opens a team-listed file in the
/// desktop app.
pub fn team_link(server: &str, file: &str) -> String {
    format!("figcraft://join?server={}&file={}&team=1", server.trim(), file.trim())
}

/// Browser link (`<web>#join=<encoded_team_link>`) that opens a team-listed file in the web build.
pub fn web_link(web: &str, server: &str, file: &str) -> String {
    format!("{}#join={}", web.trim(), percent_encode(&team_link(server, file)))
}

fn access_id(a: Access) -> &'static str {
    match a {
        Access::Edit => "edit",
        Access::Comment => "comment",
        Access::View => "view",
    }
}

impl Session {
    /// Current team file list state for the server configured in Preferences.
    pub fn team(&self) -> Option<&Team> {
        self.live.team.as_ref()
    }

    /// Ask the server in Preferences (unless unset or the public default server) for its team list.
    pub(crate) fn team_refresh(&mut self) -> Result<()> {
        let server = self.prefs.publish_server.trim().to_string();
        if server.is_empty() || same_server(&server, DEFAULT_SERVER) {
            self.live.team = None;
            return Ok(());
        }
        let req = NetRequest::Hosting { server: server.clone(), file: String::new(), owner: String::new(), req: Some(Admin::Team) };
        let (reply, _) = self.net(req)?;
        let list = match reply {
            NetReply::Hosting(Ok((_, Some(AdminReply::Team { web, files })))) => {
                let mapped = files
                    .into_iter()
                    .map(|tf| {
                        let clean = clean_name(&tf.file.name);
                        TeamFile {
                            name: if clean.is_empty() { tf.file.id.clone() } else { clean },
                            file: tf.file.id,
                            updated: tf.file.updated,
                            people: tf.file.people,
                            access: tf.access,
                        }
                    })
                    .collect();
                TeamList::Files { web: web.trim().to_string(), files: mapped }
            }
            NetReply::Hosting(Ok((_, Some(AdminReply::Failed { .. })))) => TeamList::None,
            NetReply::Hosting(Err(why)) if why.contains("couldn't be read") => TeamList::None,
            NetReply::Hosting(Err(why)) => TeamList::Offline(why),
            _ => TeamList::None,
        };
        self.live.team = Some(Team { server, list });
        Ok(())
    }

    /// JSON representation of `self.live.team` for `collab.files`.
    pub(crate) fn team_json(&self) -> Value {
        let Some(team) = &self.live.team else { return Value::Null };
        let host = server_host(&team.server);
        match &team.list {
            TeamList::None => json!({
                "server": team.server,
                "host": host,
                "state": "none",
                "reason": Value::Null,
                "web": Value::Null,
                "files": [],
            }),
            TeamList::Offline(why) => json!({
                "server": team.server,
                "host": host,
                "state": "offline",
                "reason": why,
                "web": Value::Null,
                "files": [],
            }),
            TeamList::Files { web, files } => {
                let web_opt = (!web.is_empty()).then(|| web.as_str());
                let items: Vec<Value> = files
                    .iter()
                    .map(|f| {
                        let link = team_link(&team.server, &f.file);
                        let wlink = web_opt.map(|w| web_link(w, &team.server, &f.file));
                        json!({
                            "file": f.file,
                            "name": f.name,
                            "updated": f.updated,
                            "people": f.people,
                            "access": access_id(f.access),
                            "link": link,
                            "webLink": wlink,
                        })
                    })
                    .collect();
                json!({
                    "server": team.server,
                    "host": host,
                    "state": "listed",
                    "reason": Value::Null,
                    "web": web_opt,
                    "files": items,
                })
            }
        }
    }
}
