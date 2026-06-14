//! `anvil discuss` — interactive Interlocutor session that produces a `CharterPacket`.
//!
//! Flow:
//! 1. Load project config; locate the "interlocutor" model binding.
//! 2. Ensure the sidecar is running; connect + handshake.
//! 3. Multi-turn chat loop: user types, model streams tokens, loop until Charter packet extracted.
//! 4. Render `charter.md` and write it to the project root.

use std::io::{BufRead as _, Write as _};
use std::path::Path;

use anvil_core::{
    config::load_config,
    error::AnvilError,
    pipeline::{extract_charter_packet_json, CharterPacket},
    render::render_charter_md,
};
use anvil_sidecar_client::proto::{self, invoke_request::Payload};

use crate::session::{
    connect_and_handshake, ensure_sidecar_running, find_model_binding, retrieve_api_key,
};
use crate::setup::with_tokio;

// ── Interlocutor system prompt ────────────────────────────────────────────────

const INTERLOCUTOR_SYSTEM_PROMPT: &str = "\
You are the Interlocutor for an Anvil project workflow. Your role is to conduct \
a focused discussion with the user to understand their project and produce a \
structured Charter packet.

A Charter packet captures the project's foundational decisions:
- title: the project name (required)
- goals: what the project accomplishes (required, list)
- scope: what the project covers (required)
- out_of_scope: what is explicitly excluded (optional, list)
- required_choices: key technology or design decisions that must be locked (optional, list)
- success_criteria: measurable completion conditions (required, list)
- stakeholders: who this project is for (optional, list)
- additional_notes: anything else worth capturing (optional)

Ask clarifying questions to understand the project thoroughly. When you have \
enough information to produce a complete Charter packet, output it as JSON \
wrapped in <charter_packet>...</charter_packet> tags. For example:

<charter_packet>
{
  \"title\": \"My Project\",
  \"goals\": [\"Accomplish X\", \"Enable Y\"],
  \"scope\": \"Everything needed to accomplish X and Y.\",
  \"out_of_scope\": [\"Z feature\"],
  \"required_choices\": [\"Primary language\"],
  \"success_criteria\": [\"X ships to production\", \"Y is measurable\"],
  \"stakeholders\": [\"Alice (product)\", \"Bob (eng)\"],
  \"additional_notes\": null
}
</charter_packet>

Once you produce the charter_packet, the session ends and the charter is written to disk.";

// ── Public entry point ─────────────────────────────────────────────────────────

/// Runs `anvil discuss` for the project at `project_root`.
///
/// # Errors
///
/// Returns [`AnvilError`] on config, sidecar, or file I/O failure.
pub fn run_discuss(project_root: &Path) -> Result<(), AnvilError> {
    let config = load_config(project_root)?;
    let binding = find_model_binding(&config, crate::setup::ROLE_INTERLOCUTOR)?;
    let conn_name = binding.provider_connection.clone();
    let model_id = binding.model_identity.clone();

    let conn = config
        .provider_connections
        .get(&conn_name)
        .ok_or_else(|| AnvilError::ProviderConnectionMissing(conn_name.clone()))?;
    let api_key = retrieve_api_key(&conn_name, &conn.credential_ref)?;

    let port = ensure_sidecar_running(project_root, &config)?;
    let mut client = connect_and_handshake(port, &config)?;

    println!("Anvil Interlocutor — type your response, or 'done' to produce the Charter.");
    println!("──────────────────────────────────────────────────────────────────────────");

    let mut messages: Vec<proto::Message> = Vec::new();
    let stdin = std::io::stdin();

    // First assistant turn: open the discussion.
    let first_response = with_tokio(stream_one_turn(
        &mut client,
        INTERLOCUTOR_SYSTEM_PROMPT,
        &messages,
        "Let's begin the Charter discussion. What project are you working on?",
        &model_id,
        &conn_name,
        &api_key,
    ))?;

    messages.push(proto::Message {
        role: "assistant".to_owned(),
        content: first_response.clone(),
    });

    if let Some(json) = extract_charter_packet_json(&first_response) {
        return finalize_charter(project_root, json);
    }

    // Conversation loop.
    loop {
        print!("\nYou: ");
        std::io::stdout().flush().ok();

        let mut user_input = String::new();
        let n = stdin
            .lock()
            .read_line(&mut user_input)
            .map_err(AnvilError::Io)?;
        if n == 0 {
            return Err(AnvilError::Io(std::io::Error::other(
                "stdin closed (EOF) — interactive terminal required for `anvil discuss`",
            )));
        }
        let user_input = user_input.trim().to_owned();

        if user_input.is_empty() {
            continue;
        }

        // "done" triggers a final Charter-packet request.
        let effective_input = if user_input.eq_ignore_ascii_case("done") {
            "I'm satisfied with the discussion. Please produce the Charter packet now as JSON \
             in <charter_packet>...</charter_packet> tags."
                .to_owned()
        } else {
            user_input
        };

        messages.push(proto::Message {
            role: "user".to_owned(),
            content: effective_input,
        });

        let response = with_tokio(stream_one_turn(
            &mut client,
            INTERLOCUTOR_SYSTEM_PROMPT,
            &messages,
            "",
            &model_id,
            &conn_name,
            &api_key,
        ))?;

        messages.push(proto::Message {
            role: "assistant".to_owned(),
            content: response.clone(),
        });

        if let Some(json) = extract_charter_packet_json(&response) {
            return finalize_charter(project_root, json);
        }
    }
}

// ── Streaming one turn ─────────────────────────────────────────────────────────

/// Invokes one assistant turn via `invoke_streaming`, prints tokens, and returns the full text.
///
/// When `new_user_message` is non-empty it is appended to the history for this call only
/// (caller is responsible for adding it to `messages` before the next call).
async fn stream_one_turn(
    client: &mut anvil_sidecar_client::client::AnvilSidecarClient,
    system_prompt: &str,
    history: &[proto::Message],
    new_user_message: &str,
    model_id: &str,
    conn_name: &str,
    api_key: &str,
) -> Result<String, AnvilError> {
    let mut msgs = history.to_vec();
    if !new_user_message.is_empty() {
        msgs.push(proto::Message {
            role: "user".to_owned(),
            content: new_user_message.to_owned(),
        });
    }

    let request = proto::InvokeRequest {
        idempotency_key: String::new(), // set by client
        model_id: model_id.to_owned(),
        provider_connection_id: conn_name.to_owned(),
        credentials: Some(proto::Credentials {
            credential: Some(proto::credentials::Credential::ApiKey(api_key.to_owned())),
        }),
        timeout: Some(proto::Timeout { millis: 120_000 }),
        payload: Some(Payload::Chat(proto::ChatRequest {
            system_prompt: system_prompt.to_owned(),
            messages: msgs,
            max_tokens: Some(4096),
            temperature: None,
        })),
    };

    print!("\nAnvil: ");
    std::io::stdout().flush().ok();

    let stream = client
        .invoke_streaming(request)
        .await
        .map_err(|e| AnvilError::Io(std::io::Error::other(format!("invoke_streaming: {e}"))))?;

    let final_result = stream
        .drain_displaying(|tok| {
            print!("{tok}");
            std::io::stdout().flush().ok();
        })
        .await
        .map_err(|e| AnvilError::Io(std::io::Error::other(format!("stream: {e}"))))?;

    println!(); // newline after streamed tokens

    // FinalResult is the only authoritative source; token accumulation is display-only.
    // An empty or missing FinalResult indicates a sidecar/protocol issue, not a model choice.
    let content = match final_result.result {
        Some(proto::final_result::Result::Chat(ref chat)) if !chat.content.is_empty() => {
            chat.content.clone()
        }
        Some(proto::final_result::Result::Chat(_)) => {
            return Err(AnvilError::Io(std::io::Error::other(
                "sidecar FinalResult contained empty chat content — cannot commit partial stream",
            )));
        }
        None => {
            return Err(AnvilError::Io(std::io::Error::other(
                "sidecar FinalResult was absent — stream did not complete cleanly",
            )));
        }
        Some(_) => {
            return Err(AnvilError::Io(std::io::Error::other(
                "sidecar FinalResult was not a Chat result — unexpected response variant",
            )));
        }
    };

    Ok(content)
}

// ── Charter finalization ───────────────────────────────────────────────────────

fn finalize_charter(project_root: &Path, packet_json: &str) -> Result<(), AnvilError> {
    let packet = CharterPacket::from_model_json(packet_json)?;

    packet
        .validate()
        .map_err(AnvilError::CharterPacketInvalid)?;

    let md = render_charter_md(&packet);
    let charter_path = project_root.join("charter.md");
    std::fs::write(&charter_path, md.as_bytes())?;

    println!("\n✓ Charter packet received and validated.");
    println!("  Written to: {}", charter_path.display());
    println!("  Next step: `anvil charter review` to invoke the reviewer.");
    Ok(())
}
