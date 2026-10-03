//! Exercise the public hook CLI, including event-name setup and stdout JSON.
//! Codex rejects unknown fields (notably Cursor's `user_message`):
//! https://learn.chatgpt.com/docs/hooks#userpromptsubmit

use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

fn run_hook(hook: &str, input: Value) -> Option<Value> {
    let cwd = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_contextstream-mcp"))
        .args(["hook", hook, "--contextstream-managed-hook=v1"])
        .env_clear()
        // The prompt-format probe needs no API calls or state writes. Save
        // intent remains enabled and exercises actual context output.
        .env("CONTEXTSTREAM_REMINDER_ENABLED", "false")
        .current_dir(cwd.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{hook}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    if stdout.trim().is_empty() {
        return None;
    }
    Some(serde_json::from_str(&stdout).expect("one complete JSON output, without log noise"))
}

fn assert_codex_prompt_context(output: &Value, expected_text: &str) {
    // Validate the public contract instead of merely accepting any valid JSON.
    let allowed_fields = [
        "continue",
        "stopReason",
        "systemMessage",
        "suppressOutput",
        "decision",
        "reason",
        "hookSpecificOutput",
    ];
    for key in output.as_object().unwrap().keys() {
        assert!(
            allowed_fields.contains(&key.as_str()),
            "unsupported field {key}"
        );
    }
    let specific = &output["hookSpecificOutput"];
    assert_eq!(specific["hookEventName"], "UserPromptSubmit");
    assert!(specific["additionalContext"]
        .as_str()
        .unwrap()
        .contains(expected_text));
    assert_eq!(specific.as_object().unwrap().len(), 2);
}

#[test]
fn codex_and_claude_save_prompts_emit_supported_context_json() {
    for transcript_path in [Value::Null, json!("/tmp/imported-claude-session.jsonl")] {
        let input = json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "synthetic-session",
            "turn_id": "synthetic-codex-turn",
            "transcript_path": transcript_path,
            "prompt": "Please save this decision for future reference."
        });
        let output = run_hook("on-save-intent", input).expect("save guidance");
        assert_codex_prompt_context(&output, "CONTEXTSTREAM DOCUMENT STORAGE");
    }
    let output = run_hook(
        "on-save-intent",
        json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "claude-session",
            "prompt": "Prepare a handoff for the next agent."
        }),
    )
    .expect("handoff guidance");
    assert_codex_prompt_context(&output, "handoff");
}

#[test]
fn ordinary_prompt_does_not_trigger_save_guidance() {
    assert!(run_hook(
        "on-save-intent",
        json!({
            "hook_event_name": "UserPromptSubmit",
            "prompt": "Explain how this module works."
        })
    )
    .is_none());
}

#[test]
fn prompt_cli_distinguishes_canonical_and_cursor_events() {
    assert_eq!(
        run_hook(
            "user-prompt-submit",
            json!({
                "hook_event_name": "UserPromptSubmit", "prompt": "Explain this module."
            })
        ),
        Some(json!({}))
    );
    assert_eq!(
        run_hook(
            "user-prompt-submit",
            json!({
                "hook_event_name": "beforeSubmitPrompt", "prompt": "Explain this module."
            })
        ),
        Some(json!({"continue": true}))
    );
}

#[test]
fn legacy_cursor_and_cline_save_hooks_keep_their_formats() {
    let cursor = run_hook(
        "on-save-intent",
        json!({
            "hook_event_name": "beforeSubmitPrompt", "prompt": "Save this decision."
        }),
    )
    .unwrap();
    assert_eq!(cursor["continue"], true);
    assert!(cursor["user_message"]
        .as_str()
        .unwrap()
        .contains("CONTEXTSTREAM DOCUMENT STORAGE"));
    assert!(cursor.get("hookSpecificOutput").is_none());

    let cline = run_hook(
        "on-save-intent",
        json!({
            "hookName": "UserPromptSubmit", "prompt": "Save this decision."
        }),
    )
    .unwrap();
    assert_eq!(cline["cancel"], false);
    assert!(cline["contextModification"]
        .as_str()
        .unwrap()
        .contains("CONTEXTSTREAM DOCUMENT STORAGE"));
}
