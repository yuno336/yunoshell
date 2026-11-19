use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

#[derive(Clone, Debug)]
pub enum IpcCommand {
    Toggle(String),
    Open(String),
    OpenFilesDir(String),
    OpenSettingsTab(usize, Option<String>),
    OpenControlsSection(String),
    Volume(String),
    Brightness(String),
    Agy(String),
    PortalOpen(String),
    PortalCancel(String),
    Close,
    SetBarPos(String),
    FilesMode(String),
    FilesAct(String),
    ReloadTheme,
    Quit,
}

pub fn socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(runtime).join("yunoshell.sock")
}

pub fn send_cli_command(args: &[String]) -> Result<String, String> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path)
        .map_err(|e| format!("Cannot connect to {}: {}", path.display(), e))?;
    let line = args.join(" ");
    stream
        .write_all(format!("{line}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    let _ = stream.flush();
    let mut reader = BufReader::new(stream);
    let mut resp = String::new();
    let _ = reader.read_line(&mut resp);
    Ok(resp.trim().to_string())
}

fn parse_line(line: &str) -> Option<IpcCommand> {
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }
    match parts[0] {
        "settings" if parts.len() >= 2 => {
            let tab = match parts[1] {
                "time" => Some((0, None)),
                "personalize" => Some((1, None)),
                "theme" | "bar" => Some((1, Some("theme".to_string()))),
                "mango" | "mangowm" => Some((1, Some("mango".to_string()))),
                "desktop" | "wallpaper" => Some((1, Some("desktop".to_string()))),
                "lock" | "lockscreen" => Some((1, Some("lockscreen".to_string()))),
                "login" | "sddm" => Some((1, Some("sddm".to_string()))),
                "display" | "monitors" => Some((2, None)),
                "system" => Some((3, None)),
                "controls" => Some((4, None)),
                _ => None,
            };
            match tab {
                Some((i, sub)) => Some(IpcCommand::OpenSettingsTab(i, sub)),
                None => Some(IpcCommand::Toggle("settings".to_string())),
            }
        }
        "close" => Some(IpcCommand::Close),
        "agy" if parts.len() >= 2 => Some(IpcCommand::Agy(parts[1..].join(" "))),
        "quit" | "exit" => Some(IpcCommand::Quit),
        "reload" | "reload-config" | "reload-theme" | "reload-apps" | "rescan-apps" => Some(IpcCommand::ReloadTheme),
        "bar-pos" => parts.get(1).map(|p| IpcCommand::SetBarPos((*p).to_string())),
        "controls" if parts.len() >= 2 && matches!(parts[1], "wifi" | "bt" | "bluetooth" | "audio" | "sound" | "battery" | "power") => {
            Some(IpcCommand::OpenControlsSection(parts[1].to_string()))
        }
        "vol" | "volume" if parts.len() >= 2 && !matches!(parts[1], "open" | "close" | "toggle") => {
            Some(IpcCommand::Volume(parts[1..].join(" ")))
        }
        "bri" | "bright" | "brightness" if parts.len() >= 2 && !matches!(parts[1], "open" | "close" | "toggle") => {
            Some(IpcCommand::Brightness(parts[1..].join(" ")))
        }
        "portal" if parts.len() >= 3 && parts[1] == "open" => {
            Some(IpcCommand::PortalOpen(parts[2..].join(" ")))
        }
        "portal" if parts.len() >= 3 && parts[1] == "cancel" => {
            Some(IpcCommand::PortalCancel(parts[2].to_string()))
        }
        "files-mode" if parts.len() >= 2 => {
            Some(IpcCommand::FilesMode(parts[1].to_string()))
        }
        "files-act" if parts.len() >= 2 => {
            Some(IpcCommand::FilesAct(parts[1..].join(" ")))
        }
        "files" => {
            let act = parts.get(1).copied().unwrap_or("toggle");
            match act {
                "mode" if parts.len() >= 3 => {
                    Some(IpcCommand::FilesMode(parts[2].to_string()))
                }
                "open" if parts.len() >= 3 => {
                    Some(IpcCommand::OpenFilesDir(parts[2..].join(" ")))
                }
                "open" => Some(IpcCommand::Open("files".to_string())),
                "openDir" if parts.len() >= 3 => {
                    Some(IpcCommand::OpenFilesDir(parts[2..].join(" ")))
                }
                "close" => Some(IpcCommand::Close),
                _ => {
                    if parts.len() >= 2 && !matches!(parts[1], "toggle" | "close") {
                        Some(IpcCommand::OpenFilesDir(parts[1..].join(" ")))
                    } else {
                        Some(IpcCommand::Toggle("files".to_string()))
                    }
                }
            }
        }
        "launcher" | "controls" | "power" | "settings" | "media" | "notifications"
        | "volume" | "brightness" | "battery" | "wifi" | "bluetooth" | "antigravity" => {
            let target = parts[0].to_string();
            let act = parts.get(1).copied().unwrap_or("toggle");
            match act {
                "open" => Some(IpcCommand::Open(target)),
                "close" => Some(IpcCommand::Close),
                "openDir" if parts.len() >= 3 => {
                    Some(IpcCommand::OpenFilesDir(parts[2..].join(" ")))
                }
                _ => Some(IpcCommand::Toggle(target)),
            }
        }
        "toggle" => parts.get(1).map(|t| IpcCommand::Toggle((*t).to_string())),
        "open" => parts.get(1).map(|t| IpcCommand::Open((*t).to_string())),
        _ => None,
    }
}

pub fn start_ipc_server() -> Receiver<IpcCommand> {
    let (tx, rx): (Sender<IpcCommand>, Receiver<IpcCommand>) = mpsc::channel();
    let path = socket_path();
    let _ = fs::remove_file(&path);

    thread::spawn(move || {
        let Ok(listener) = UnixListener::bind(&path) else {
            eprintln!("[yunoshell] failed to bind IPC socket at {}", path.display());
            return;
        };
        for stream in listener.incoming().flatten() {
            let tx_c = tx.clone();
            thread::spawn(move || {
                let mut stream = stream;
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).is_ok() {
                    if let Some(cmd) = parse_line(&line) {
                        let _ = tx_c.send(cmd);
                        let _ = stream.write_all(b"ok\n");
                    } else {
                        let _ = stream.write_all(b"err: unknown command\n");
                    }
                }
            });
        }
    });
    rx
}
