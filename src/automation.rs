//! Opt-in local IPC. Only the UI thread owns/mutates the live editor.
use crate::{Message, document::Document};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    sync::mpsc,
    time::Duration,
};

pub const MAX_MESSAGE: u64 = 24 * 1024 * 1024;
pub struct Snapshot {
    pub document: Document,
    pub revision: u64,
    pub phase: f32,
}
pub enum Request {
    State(mpsc::Sender<Result<Value, String>>),
    Dispatch {
        action: crate::editor::actions::Action,
        expected_revision: Option<u64>,
        reply: mpsc::Sender<Result<crate::editor::actions::ActionReceipt, String>>,
    },
    Snapshot(mpsc::Sender<Result<Snapshot, String>>),
    Apply {
        document: Document,
        revision: u64,
        replace: bool,
        reply: mpsc::Sender<Result<Value, String>>,
    },
    Show(mpsc::Sender<Result<Value, String>>),
}
pub fn directory() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is unavailable")?;
    let dir = PathBuf::from(home).join("Library/Caches/sh.glance.desktop/automation");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    Ok(dir)
}
pub fn socket() -> Result<PathBuf, String> {
    Ok(directory()?.join("editor.sock"))
}
pub fn state(snapshot: &Snapshot) -> Value {
    let d = &snapshot.document;
    json!({"revision":snapshot.revision,"width":d.base.width(),"height":d.base.height(),
        "coordinate_space":"source image pixels; backdrop padding excluded", "backdrop":d.backdrop,"image_animation":d.image_animation,
        "objects":d.marks.iter().enumerate().map(|(i,m)| json!({"id":format!("{}:{i}",snapshot.revision),"mark":m})).collect::<Vec<_>>()})
}
fn wait<T>(
    sender: &async_channel::Sender<Message>,
    make: impl FnOnce(mpsc::Sender<Result<T, String>>) -> Request,
) -> Result<T, String> {
    let (tx, rx) = mpsc::channel();
    sender
        .send_blocking(Message::Automation(make(tx)))
        .map_err(|_| "Editor closed".to_string())?;
    rx.recv_timeout(Duration::from_secs(120))
        .map_err(|e| e.to_string())?
}
pub fn dispatch(
    sender: &async_channel::Sender<Message>,
    name: &str,
    args: Value,
) -> Result<Value, String> {
    if name == "get_editor_state" {
        crate::mcp::validate_tool(name, &args)?;
        return wait(sender, Request::State);
    }
    if name == "dispatch_action" {
        crate::mcp::validate_tool(name, &args)?;
        let action = crate::editor::actions::Action::from_json(args["action"].clone())?;
        let receipt = wait(sender, |reply| Request::Dispatch {
            action,
            expected_revision: args.get("expected_revision").and_then(Value::as_u64),
            reply,
        })?;
        return serde_json::to_value(receipt).map_err(|e| e.to_string());
    }
    if name == "open_editor" {
        return wait(sender, Request::Show);
    }
    let mut snapshot = wait(sender, Request::Snapshot)?;
    if let Some(expected) = args.get("expected_revision").and_then(Value::as_u64)
        && expected != snapshot.revision
    {
        return Err("Stale expected_revision; read state and retry".into());
    }
    let (result, changed, replace) = crate::mcp::operate(name, &args, &mut snapshot)?;
    if changed {
        wait(sender, |reply| Request::Apply {
            document: snapshot.document,
            revision: snapshot.revision,
            replace,
            reply,
        })
    } else {
        Ok(result)
    }
}
pub fn read_line(reader: &mut impl BufRead) -> Result<Option<String>, String> {
    let mut bytes = Vec::new();
    let n = reader
        .take(MAX_MESSAGE + 1)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Ok(None);
    }
    if n as u64 > MAX_MESSAGE {
        return Err("Message exceeds 24 MiB limit".into());
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|e| e.to_string())
}
pub fn listen(sender: async_channel::Sender<Message>) -> Result<(), String> {
    let path = socket()?;
    if path.exists() {
        if UnixStream::connect(&path).is_ok() {
            return Err("Another automation editor is already running".into());
        }
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    let listener = UnixListener::bind(&path).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            serve(&mut stream, &sender);
        }
        let _ = std::fs::remove_file(path);
    });
    Ok(())
}
fn serve(stream: &mut UnixStream, sender: &async_channel::Sender<Message>) {
    let result = (|| {
        let line = read_line(&mut BufReader::new(&*stream))?.ok_or("Empty request")?;
        let request: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        dispatch(
            sender,
            request["name"].as_str().ok_or("Missing tool name")?,
            request["arguments"].clone(),
        )
    })();
    let response = match result {
        Ok(v) => json!({"result":v}),
        Err(e) => json!({"error":e}),
    };
    let _ = writeln!(stream, "{response}");
}
pub fn call(name: &str, args: Value) -> Result<Value, String> {
    let mut stream = UnixStream::connect(socket()?).map_err(|_| {
        "Native editor unavailable. Launch Glance with --automation, then call open_editor."
            .to_string()
    })?;
    stream
        .set_read_timeout(Some(Duration::from_secs(180)))
        .map_err(|e| e.to_string())?;
    writeln!(stream, "{}", json!({"name":name,"arguments":args})).map_err(|e| e.to_string())?;
    let line = read_line(&mut BufReader::new(stream))?.ok_or("Editor disconnected")?;
    let response: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
    if let Some(error) = response["error"].as_str() {
        Err(error.into())
    } else {
        Ok(response["result"].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unix_transport_dispatches_mutations_and_reports_conflicts() {
        let (sender, receiver) = async_channel::unbounded();
        let worker = std::thread::spawn(move || {
            let mut d = Document::new(image::RgbaImage::from_pixel(
                20,
                20,
                image::Rgba([0, 0, 0, 255]),
            ));
            let mut revision = 0;
            while let Ok(Message::Automation(request)) = receiver.recv_blocking() {
                match request {
                    Request::State(reply) => {
                        let _ = reply.send(Ok(json!({"revision":revision})));
                    }
                    Request::Dispatch { reply, .. } => {
                        let _ = reply.send(Err("No live editor in this transport fixture".into()));
                    }
                    Request::Snapshot(reply) => {
                        reply
                            .send(Ok(Snapshot {
                                document: d.clone(),
                                revision,
                                phase: 0.,
                            }))
                            .unwrap();
                    }
                    Request::Apply {
                        document, reply, ..
                    } => {
                        d = document;
                        revision += 1;
                        reply
                            .send(Ok(state(&Snapshot {
                                document: d.clone(),
                                revision,
                                phase: 0.,
                            })))
                            .unwrap();
                    }
                    Request::Show(reply) => {
                        reply.send(Ok(json!({"native_window":true}))).unwrap();
                    }
                }
            }
        });
        for (name, args, success) in [
            (
                "add_annotation",
                json!({"mark":{"tool":"rectangle","points":[[2,2],[15,15]],"width":2,"text":"","color":[255,0,0,255]}}),
                true,
            ),
            ("get_document", json!({}), true),
            ("delete_annotation", json!({"id":"0:0"}), false),
            ("delete_annotation", json!({"id":"1:0"}), true),
        ] {
            let (mut client, mut server) = UnixStream::pair().unwrap();
            let tx = sender.clone();
            let connection = std::thread::spawn(move || serve(&mut server, &tx));
            writeln!(client, "{}", json!({"name":name,"arguments":args})).unwrap();
            let line = read_line(&mut BufReader::new(client)).unwrap().unwrap();
            let value: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(value.get("error").is_none(), success, "{value}");
            connection.join().unwrap();
        }
        drop(sender);
        worker.join().unwrap();
    }
}
