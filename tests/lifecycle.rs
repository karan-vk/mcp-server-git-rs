//! Process-level lifecycle behaviour of the stdio server binary.

use std::io::Write;
use std::process::{Command, Stdio};

fn spawn() -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_mcp-server-git-rs"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn server binary")
}

#[test]
fn input_closed_before_any_client_message_is_a_clean_exit() {
    let mut child = spawn();
    // Host shuts the unused server down: stdin closes with nothing sent.
    drop(child.stdin.take());
    let out = child.wait_with_output().expect("wait for server");
    assert!(
        out.status.success(),
        "expected exit 0, got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn malformed_first_message_still_fails() {
    let mut child = spawn();
    {
        let mut stdin = child.stdin.take().expect("stdin");
        // A notification is not a valid way to open a session.
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
        )
        .expect("write");
    }
    let out = child.wait_with_output().expect("wait for server");
    assert!(
        !out.status.success(),
        "a protocol violation must not be swallowed as a clean exit"
    );
}
