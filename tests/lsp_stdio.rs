use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

fn frame(value: Value) -> Vec<u8> {
    let body = serde_json::to_vec(&value).unwrap();
    let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    frame.extend(body);
    frame
}

fn decode(mut bytes: &[u8]) -> Vec<Value> {
    let mut messages = Vec::new();
    while !bytes.is_empty() {
        let header_end = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("LSP header terminator");
        let header = std::str::from_utf8(&bytes[..header_end]).unwrap();
        let length = header
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .expect("Content-Length");
        let body_start = header_end + 4;
        let body_end = body_start + length;
        messages.push(serde_json::from_slice(&bytes[body_start..body_end]).unwrap());
        bytes = &bytes[body_end..];
    }
    messages
}

#[test]
fn real_stdio_process_completes_lsp_lifecycle() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mimispec"))
        .args(["lsp", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn mimispec lsp");
    let mut input = Vec::new();
    for message in [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "initializationOptions": { "collaborationMode": "advisory" } } }),
        json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }),
        json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": { "textDocument": { "uri": "file:///stdio.mms", "languageId": "mimispec", "version": 1, "text": "desc?? \"stdio test\"\n" } } }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "mimispec/documentSnapshot", "params": { "textDocument": { "uri": "file:///stdio.mms" } } }),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "mimispec/prepareQueueBatch", "params": { "uri": "file:///stdio.mms", "base_version": 1, "actor": "human", "slot_ids": [0], "target": "?" } }),
        json!({ "jsonrpc": "2.0", "id": 4, "method": "shutdown", "params": null }),
        json!({ "jsonrpc": "2.0", "method": "exit", "params": null }),
    ] {
        input.extend(frame(message));
    }
    child.stdin.take().unwrap().write_all(&input).unwrap();
    let output = child.wait_with_output().expect("wait for LSP");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let messages = decode(&output.stdout);
    assert!(messages.iter().any(|message| {
        message["id"] == 1 && message["result"]["mimispec"]["schemaVersion"] == "mimispec.ls/0.3"
    }));
    assert!(messages.iter().any(|message| {
        message["id"] == 2
            && message["result"]["session"]["mode"] == "advisory"
            && message["result"]["delegation_queue"]
                .as_array()
                .is_some_and(|queue| queue.len() == 1)
            && message["result"]["queue_tree"]["root"]["delegation_count"] == 1
    }));
    assert!(messages.iter().any(|message| {
        message["id"] == 3
            && message["result"]["accepted"] == true
            && message["result"]["workspace_edit"]["changes"]["file:///stdio.mms"]
                .as_array()
                .is_some_and(|edits| edits.len() == 1)
    }));
}
