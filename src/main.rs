extern crate env_logger;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

use std::time::Duration;
use std::io::{self, BufRead, Read, Write};
use std::thread;

use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Notification {
    jsonrpc: String,
    method: String,
    params: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum Response {
    Response {
        jsonrpc: String,
        id: Value,
        result: Value,
    },
    Error {
        jsonrpc: String,
        id: Value,
        error: RpcError,
    },
    Notification(Notification),
}

fn ver() -> String {
    "2.0".to_owned()
}

impl Response {
    fn response(msg: &InputMsg, result: Value) -> Option<Response> {
        match *msg {
            InputMsg::Rpc { ref id, .. } => Some(Response::Response {
                jsonrpc: ver(),
                id: id.clone(),
                result,
            }),
            InputMsg::Notification(_) => {
                error!("Expected method, got notification, not answering");
                None
            }
        }
    }
    fn unimplemented(msg: &InputMsg) -> Option<Response> {
        match *msg {
            InputMsg::Rpc { ref id, .. } => Some(Response::Error {
                jsonrpc: ver(),
                id: id.clone(),
                error: RpcError {
                    code: -32601,
                    message: "Method not found".to_owned(),
                },
            }),
            InputMsg::Notification(_) => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum InputMsg {
    Rpc {
        jsonrpc: String,
        id: Value,
        method: String,
        params: Value,
    },
    Notification(Notification),
}

impl InputMsg {
    fn method(&self) -> &str {
        match *self {
            InputMsg::Rpc { ref method, .. } |
            InputMsg::Notification(Notification { ref method, .. }) => method,
        }
    }
    fn time(&self) -> Duration {
        let msec = match self.method() {
            "initialize" |
            "textDocument/didOpen" => 100,
            "textDocument/didChange" => 200,
            "textDocument/didSave" => 400,
            "textDocument/completion" => 300,
            _ => 0,
        };
        Duration::from_millis(msec)
    }
    fn response(&self) -> Option<Response> {
        let method = self.method();
        match method {
            "initialize" => Response::response(self, json!({
                "capabilities": {
                    "textDocumentSync": 2,
                    "hoverProvider": true,
                    "completionProvider": {
                        "resolveProvider": true,
                        "triggerCharacters": [".", ":", "->"],
                    },
                    "definitionProvider": true,
                    "referencesProvider": true,
                    "documentHighlightProvider": true,
                    "documentSymbolProvider": true,
                    "workspaceSymbolProvider": true,
                    "codeActionProvider": true,
                    "documentFormattingProvider": true,
                    "documentRangeFormattingProvider": false,
                    "renameProvider": true,
                    "executeCommandProvider": {
                        "commands": [],
                    }
                }
            })),
            _ => {
                warn!("Unknown method {}", method);
                Response::unimplemented(self)
            }
        }
    }
}

fn main() {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    loop {
        // Ugly and expects there's only just that one header.
        let mut line = String::new();
        stdin.read_line(&mut line).expect("Couldn't read length");
        let len = line.split(' ')
            .nth(1)
            .expect("Malformed header")
            .parse()
            .expect("Malformed length");
        stdin.read_line(&mut line).expect("Couldn't read newline");
        let data = (&mut stdin).take(len);
        let inmsg: Result<InputMsg, _> = serde_json::from_reader(data);
        match inmsg {
            Err(e) => error!("Malformed input thing: {}", e),
            Ok(inmsg) => {
                let time = inmsg.time();
                debug!("Received msg {:?}, going to sleep for {:?}", inmsg, time);
                thread::sleep(time);
                if let Some(response) = inmsg.response() {
                    debug!("Providing response {:?}", response);
                    let formatted = serde_json::to_vec(&response).expect("Couldn't format response");
                    println!("Content-Length: {}", formatted.len());
                    println!();
                    io::stdout().write_all(&formatted).expect("Failed to write response");
                }
            }
        }
    }
}
