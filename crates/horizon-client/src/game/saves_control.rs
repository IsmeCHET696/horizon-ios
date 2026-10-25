//! Worlds and the multiplayer server list from the control socket: what the Select World, Edit
//! World and Multiplayer screens do, by world folder. Changing a world needs the title screen
//! (no world open), as those screens do; deleting one needs `--confirm`.
use super::Game;
use horizon_client::{Result, title};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::mpsc::Sender,
    time::{Duration, UNIX_EPOCH},
};

pub(super) const HELP: &[&str] = &[
    "worlds                         the saved worlds, newest first: folder, name, mode, last played, version",
    "world-rename FOLDER NAME       rename a world (Edit World; its folder stays)",
    "world-backup FOLDER            zip a world into backups/ (Edit World > Make Backup)",
    "world-delete FOLDER --confirm  delete a world for good (Select World > Delete)",
    "world-create [--seed S] [--creative] NAME  create a world and open it (Create New World)",
    "servers                        the multiplayer server list (servers.dat): name, address and icon",
    "server-add ADDRESS NAME        add a server to the list (Add Server)",
    "server-remove INDEX            remove the server at INDEX of servers (Delete)",
    "server-ping ADDRESS            a server's status: motd, players, version, latency and icon",
];

fn saves() -> Result<PathBuf> {
    Ok(std::env::current_dir()?.join("saves"))
}

impl Game<'_> {
    /// Some(reply) for these commands (Some(None): the reply comes later), None for others.
    pub(super) fn saves_control(
        &mut self,
        words: &[&str],
        line: &str,
        reply: &Sender<String>,
    ) -> Result<Option<Option<Value>>> {
        let folder = || {
            words
                .get(1)
                .copied()
                .ok_or_else(|| format!("{} FOLDER: the folder from worlds", words[0]))
        };
        let on_title = || {
            if self.title.is_some() {
                Ok(())
            } else {
                Err(format!(
                    "{} needs the title screen: save and leave the world first (pause, then Save and Quit to Title)",
                    words[0]
                ))
            }
        };
        let value = match words[0] {
            "worlds" => {
                let worlds: Vec<Value> = title::list_worlds(&saves()?)
                    .iter()
                    .map(|w| {
                        json!({
                            "folder": w.folder,
                            "name": w.name,
                            "mode": if w.creative { "creative" } else { "survival" },
                            "last_played": w.modified.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64),
                            "commands": w.commands,
                            "version": w.version,
                        })
                    })
                    .collect();
                json!({ "worlds": worlds })
            }
            "world-rename" => {
                on_title()?;
                let folder = folder()?;
                let name = line
                    .trim_start()
                    .strip_prefix("world-rename")
                    .and_then(|rest| rest.trim_start().strip_prefix(folder))
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                    .ok_or("world-rename FOLDER NAME: missing the new name")?;
                title::world_management::rename(&saves()?, folder, name)?;
                if let Some(title) = self.title.as_mut() {
                    title.worlds_changed();
                }
                json!({ "ok": true, "folder": folder, "name": name })
            }
            "world-delete" => {
                on_title()?;
                let folder = folder()?;
                if !words.contains(&"--confirm") {
                    return Err(format!(
                        "world-delete deletes {folder} for good: ask the player, then add --confirm"
                    )
                    .into());
                }
                title::world_management::delete(&saves()?, folder)?;
                if let Some(title) = self.title.as_mut() {
                    title.worlds_changed();
                }
                json!({ "ok": true, "deleted": folder })
            }
            "world-backup" => {
                on_title()?;
                let folder = folder()?.to_owned();
                let saves = saves()?;
                let backups = title::world_management::backups_path(&saves);
                // A big world takes a while to zip: off the game thread, as Make Backup does.
                let reply = reply.clone();
                std::thread::spawn(move || {
                    let answer = match title::world_management::backup(&saves, &folder) {
                        Ok(bytes) => {
                            json!({ "ok": true, "folder": folder, "backups": backups, "bytes": bytes })
                        }
                        Err(e) => json!({ "error": format!("backup of {folder} failed: {e}") }),
                    };
                    let _ = reply.send(answer.to_string());
                });
                return Ok(Some(None));
            }
            "world-create" => {
                on_title()?;
                let mut seed = None;
                let mut creative = false;
                let mut rest = line.trim_start().strip_prefix("world-create").unwrap_or("");
                loop {
                    rest = rest.trim_start();
                    if let Some(after) = rest.strip_prefix("--creative") {
                        creative = true;
                        rest = after;
                    } else if let Some(after) = rest.strip_prefix("--seed") {
                        let after = after.trim_start();
                        let end = after.find(char::is_whitespace).unwrap_or(after.len());
                        seed = title::parse_seed(&after[..end]);
                        rest = &after[end..];
                    } else {
                        break;
                    }
                }
                let name = rest.trim();
                let name = if name.is_empty() { "New World" } else { name };
                let folder = title::new_world_folder(&saves()?, name);
                self.open_world(&folder, Some(name), seed, creative, None)?;
                json!({ "ok": true, "folder": folder, "name": name })
            }
            "servers" => {
                let file = horizon_client::multiplayer::servers_file(&saves()?);
                let servers: Vec<Value> = horizon_protocol::servers::load(&file)
                    .iter()
                    .map(|s| json!({ "name": s.name, "address": s.ip, "icon": s.icon }))
                    .collect();
                json!({ "servers": servers })
            }
            "server-add" => {
                let address = words.get(1).ok_or("server-add ADDRESS NAME")?;
                let name = line
                    .trim_start()
                    .strip_prefix("server-add")
                    .and_then(|rest| rest.trim_start().strip_prefix(address))
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                    .unwrap_or("Minecraft Server");
                let file = horizon_client::multiplayer::servers_file(&saves()?);
                let mut list = horizon_protocol::servers::load(&file);
                list.push(horizon_protocol::servers::ServerEntry::new(name, address));
                horizon_protocol::servers::save(&file, &list)?;
                json!({ "ok": true, "index": list.len() - 1 })
            }
            "server-remove" => {
                let index: usize = words.get(1).ok_or("server-remove INDEX")?.parse()?;
                let file = horizon_client::multiplayer::servers_file(&saves()?);
                let mut list = horizon_protocol::servers::load(&file);
                if index >= list.len() {
                    return Err(
                        format!("no server at {index}; servers lists {}", list.len()).into(),
                    );
                }
                let removed = list.remove(index);
                horizon_protocol::servers::save(&file, &list)?;
                json!({ "ok": true, "removed": removed.ip })
            }
            "server-ping" => {
                let address = (*words.get(1).ok_or("server-ping ADDRESS")?).to_owned();
                // A status ping takes up to its timeout: off the game thread, as the server list does.
                let reply = reply.clone();
                std::thread::spawn(move || {
                    let answer = match horizon_client::net::ping(&address, Duration::from_secs(5)) {
                        Ok((status, ms)) => {
                            let motd = horizon_client::component::to_text(
                                &status["description"],
                                &mut |s| horizon_client::text::Text::literal(s),
                            );
                            json!({
                                "ok": true,
                                "address": address,
                                "motd": motd.plain(),
                                "online": status["players"]["online"],
                                "max": status["players"]["max"],
                                "version": status["version"]["name"],
                                "ms": ms as u64,
                                "icon": status["favicon"].as_str()
                                    .and_then(|url| url.strip_prefix("data:image/png;base64,"))
                                    .map(|b| b.replace('\n', "")),
                            })
                        }
                        Err(e) => json!({ "error": e, "address": address }),
                    };
                    let _ = reply.send(answer.to_string());
                });
                return Ok(Some(None));
            }
            _ => return Ok(None),
        };
        Ok(Some(Some(value)))
    }
}
