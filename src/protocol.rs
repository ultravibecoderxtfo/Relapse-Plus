use serde_json::{Value, json};
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

const PROTOCOL_VERSION: u64 = 1;
const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
const RECONNECT_SECONDS: u64 = 1;

#[derive(Debug, Clone)]
pub enum Event {
    State {
        state: &'static str,
        error: Option<String>,
        socket: PathBuf,
        ready: bool,
        session: i64,
    },
    Output {
        text: String,
    },
    Notice(String),
    Error(String),
}

enum Command {
    Reconnect,
    Execute { name: String, source: String },
    Stop,
}

pub struct ProtocolClient {
    tx: Sender<Command>,
    rx: Receiver<Event>,
}

impl ProtocolClient {
    pub fn new(path: PathBuf) -> Self {
        let (tx, commands) = mpsc::channel();
        let (events, rx) = mpsc::channel();
        thread::spawn(move || worker(path, commands, events));
        Self { tx, rx }
    }

    pub fn execute(&self, name: &str, source: &str) {
        let _ = self.tx.send(Command::Execute {
            name: name.to_string(),
            source: source.to_string(),
        });
    }

    pub fn reconnect(&self) {
        let _ = self.tx.send(Command::Reconnect);
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Command::Stop);
    }

    pub fn try_recv(&self) -> Option<Event> {
        self.rx.try_recv().ok()
    }
}

pub fn profile_sockets() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let uid = fs::metadata("/proc/self").map(|m| m.uid()).unwrap_or(0);
    let Ok(entries) = fs::read_dir("/tmp") else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let valid = name == "relapse.sock"
            || (name.starts_with("relapse-") && name.ends_with(".sock"));
        if !valid {
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_socket() && meta.uid() == uid {
            out.push(path);
        }
    }
    out.sort_by_key(|p| (p.file_name().map(|n| n != "relapse.sock"), p.clone()));
    out
}

pub fn profile_name(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("relapse.sock");
    if name == "relapse.sock" {
        "Default".to_string()
    } else if name.starts_with("relapse-") && name.ends_with(".sock") {
        name["relapse-".len()..name.len() - ".sock".len()].to_string()
    } else {
        path.file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("Relapse")
            .to_string()
    }
}

pub fn default_socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os("RELAPSE_SOCKET") {
        return PathBuf::from(path);
    }
    profile_sockets()
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from("/tmp/relapse.sock"))
}

fn emit_state(
    events: &Sender<Event>,
    state: &'static str,
    error: Option<String>,
    path: &Path,
    ready: bool,
    session: i64,
) {
    let _ = events.send(Event::State {
        state,
        error,
        socket: path.to_path_buf(),
        ready,
        session,
    });
}

fn wait_retry(cmds: &Receiver<Command>, events: &Sender<Event>) -> bool {
    let deadline = Instant::now() + Duration::from_secs(RECONNECT_SECONDS);
    loop {
        let now = Instant::now();
        if now >= deadline {
            return false;
        }
        match cmds.recv_timeout(deadline.saturating_duration_since(now)) {
            Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return true,
            Ok(Command::Reconnect) => {}
            Ok(Command::Execute { .. }) => {
                let _ = events.send(Event::Error(
                    "No Relapse profile socket is connected.".to_string(),
                ));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => return false,
        }
    }
}

fn worker(path: PathBuf, cmds: Receiver<Command>, events: Sender<Event>) {
    loop {
        loop {
            match cmds.try_recv() {
                Ok(Command::Stop) | Err(TryRecvError::Disconnected) => return,
                Ok(Command::Reconnect) => {}
                Ok(Command::Execute { .. }) => {
                    let _ = events.send(Event::Error(
                        "No Relapse profile socket is connected.".to_string(),
                    ));
                }
                Err(TryRecvError::Empty) => break,
            }
        }

        emit_state(&events, "connecting", None, &path, false, 0);
        let (mut stream, limit) = match connect_stream(&path) {
            Ok(connection) => connection,
            Err(error) => {
                emit_state(
                    &events,
                    "disconnected",
                    Some(error),
                    &path,
                    false,
                    0,
                );
                if wait_retry(&cmds, &events) {
                    return;
                }
                continue;
            }
        };

        let mut ready = false;
        let mut session = 0i64;
        let mut next_script_id = 1u64;
        let mut buffer = Vec::new();
        emit_state(&events, "waiting", None, &path, false, 0);

        'connected: loop {
            let mut reconnect = false;
            loop {
                match cmds.try_recv() {
                    Ok(Command::Stop) | Err(TryRecvError::Disconnected) => return,
                    Ok(Command::Reconnect) => {
                        reconnect = true;
                        break;
                    }
                    Ok(Command::Execute { name, source }) => {
                        if !ready || session <= 0 {
                            let _ = events.send(Event::Error(
                                "Relapse is connected, but no game session is ready."
                                    .to_string(),
                            ));
                            continue;
                        }
                        let frame = json!({
                            "op": "execute",
                            "script_id": next_script_id,
                            "session": session,
                            "name": name.chars().take(255).collect::<String>(),
                            // Ask the backend to stream the script's console
                            // output back to us instead of keeping it in-game.
                            "redirect_console": true,
                            "source": source,
                        });
                        next_script_id += 1;
                        if let Err(error) = send_frame(&mut stream, &frame, limit) {
                            let _ = events.send(Event::Error(error));
                            reconnect = true;
                            break;
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
            if reconnect {
                break 'connected;
            }

            let message = match read_message(&mut stream, &mut buffer, limit) {
                Ok(Some(message)) => message,
                Ok(None) => continue,
                Err(error) => {
                    emit_state(
                        &events,
                        "disconnected",
                        Some(friendly_io(&error)),
                        &path,
                        false,
                        0,
                    );
                    break 'connected;
                }
            };

            match message.get("type").and_then(Value::as_str) {
                Some("hello") | Some("status") => {
                    let is_ready = message.get("ready").and_then(Value::as_bool);
                    let session_value = message.get("session").and_then(Value::as_i64);
                    match (is_ready, session_value) {
                        (Some(is_ready), Some(session_value))
                            if session_value >= 0 && (!is_ready || session_value > 0) =>
                        {
                            ready = is_ready;
                            session = session_value;
                            emit_state(
                                &events,
                                if ready { "ready" } else { "waiting" },
                                None,
                                &path,
                                ready,
                                session,
                            );
                        }
                        _ => {
                            let _ = events.send(Event::Error(
                                "Relapse sent an invalid hello/status frame.".to_string(),
                            ));
                        }
                    }
                }
                Some("exec_result") => {
                    let id = message
                        .get("script_id")
                        .and_then(Value::as_i64)
                        .unwrap_or(0);
                    let phase = message
                        .get("phase")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if phase == "submitted" {
                        let _ = events.send(Event::Notice(format!(
                            "submitted script {id}"
                        )));
                    } else {
                        let detail = message
                            .get("detail")
                            .and_then(Value::as_str)
                            .unwrap_or("Relapse rejected the script.");
                        let _ = events.send(Event::Error(detail.to_string()));
                    }
                }
                Some("log") => {
                    let text = message
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if !text.is_empty() {
                        let _ = events.send(Event::Output {
                            text: text.trim_end().to_string(),
                        });
                    }
                }
                Some("error") => {
                    let detail = message
                        .get("detail")
                        .and_then(Value::as_str)
                        .unwrap_or("Relapse reported an error.");
                    let _ = events.send(Event::Error(detail.to_string()));
                }
                Some(_) | None => {}
            }
        }
    }
}

fn connect_stream(path: &Path) -> Result<(UnixStream, usize), String> {
    let mut stream =
        UnixStream::connect(path).map_err(|error| friendly_error(&error, path))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    send_frame(
        &mut stream,
        &json!({"version": PROTOCOL_VERSION, "channel": "@main"}),
        4096,
    )?;
    let mut buffer = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(2);
    let opening = loop {
        match read_message(&mut stream, &mut buffer, 4096) {
            Ok(Some(message)) => break message,
            Ok(None) => {
                if Instant::now() >= deadline {
                    return Err("Relapse did not answer the channel handshake.".to_string());
                }
            }
            Err(error) => return Err(friendly_io(&error)),
        }
    };
    if opening.get("ok").and_then(Value::as_bool) != Some(true) {
        let detail = opening
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("channel rejected");
        return Err(format!("Relapse rejected the channel: {detail}"));
    }
    if opening.get("version").and_then(Value::as_u64) != Some(PROTOCOL_VERSION) {
        return Err("Unsupported Relapse protocol version.".to_string());
    }
    let limit = opening
        .get("max_message_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(MAX_MESSAGE_BYTES as u64)
        .min(MAX_MESSAGE_BYTES as u64) as usize;
    if limit == 0 {
        return Err("Relapse returned an invalid message limit.".to_string());
    }
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| e.to_string())?;
    Ok((stream, limit))
}

fn send_frame(stream: &mut UnixStream, value: &Value, limit: usize) -> Result<(), String> {
    let body = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if body.len() > limit {
        return Err("Relapse frame exceeds the message limit.".to_string());
    }
    let length = (body.len() as u32).to_be_bytes();
    stream
        .write_all(&length)
        .and_then(|_| stream.write_all(&body))
        .and_then(|_| stream.flush())
        .map_err(|e| e.to_string())
}

fn read_message(
    stream: &mut UnixStream,
    buffer: &mut Vec<u8>,
    limit: usize,
) -> io::Result<Option<Value>> {
    loop {
        if buffer.len() >= 4 {
            let length = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]])
                as usize;
            if length == 0 || length > limit {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Relapse sent an invalid frame size",
                ));
            }
            if buffer.len() >= 4 + length {
                let body: Vec<u8> = buffer[4..4 + length].to_vec();
                buffer.drain(..4 + length);
                let message = serde_json::from_slice(&body).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Relapse sent malformed JSON",
                    )
                })?;
                return Ok(Some(message));
            }
        }

        let mut chunk = [0u8; 8192];
        match stream.read(&mut chunk) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Relapse closed the socket",
                ));
            }
            Ok(read) => buffer.extend_from_slice(&chunk[..read]),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(error),
        }
    }
}

fn friendly_error(error: &io::Error, path: &Path) -> String {
    match error.kind() {
        io::ErrorKind::NotFound => format!("No Relapse socket at {}", path.display()),
        io::ErrorKind::PermissionDenied => {
            format!("Permission denied at {}", path.display())
        }
        io::ErrorKind::ConnectionRefused => {
            format!("Relapse refused the connection at {}", path.display())
        }
        _ => friendly_io(error),
    }
}

fn friendly_io(error: &io::Error) -> String {
    error.to_string()
}
