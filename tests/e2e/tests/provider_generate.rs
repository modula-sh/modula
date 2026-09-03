//! E2E: `ProviderService.Generate` — one throwaway provider session per call,
//! returning the concatenated deltas.

use anyhow::Result;
use modula_rpc::v1::GenerateTextRequest;
use modula_test_support::fixtures as common;
use modula_test_support::Harness;
use tonic::Code;

fn request(ws: &str, provider_id: &str, instruction: &str) -> GenerateTextRequest {
    GenerateTextRequest {
        workspace_id: ws.to_string(),
        provider_id: provider_id.to_string(),
        model: None,
        instruction: instruction.to_string(),
        field_label: Some("Description".to_string()),
    }
}

/// `runtime_from_provider` validates `config_dir` on disk, so every case needs a
/// real directory to point the claude provider at.
async fn provider(h: &Harness, ws: &str) -> Result<String> {
    let cfg_dir = h.modula_dir.join("fake-claude");
    std::fs::create_dir_all(&cfg_dir)?;
    common::create_provider(h, ws, "Claude", &cfg_dir).await
}

#[tokio::test]
async fn generate_returns_joined_deltas() -> Result<()> {
    let recipe = serde_json::json!({
        "stream": [
            {"type": "system", "subtype": "init", "session_id": "gen-session"},
            {"type": "stream_event", "event": {"type": "content_block_delta",
                "delta": {"text": "Implement the "}}},
            {"type": "stream_event", "event": {"type": "content_block_delta",
                "delta": {"text": "widget."}}},
            {"type": "result", "subtype": "success"}
        ]
    })
    .to_string();
    let h = Harness::start_with_env(&[("MODULA_MOCK_RECIPE", &recipe)]).await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let provider_id = provider(&h, &ws).await?;

    let text = h
        .providers()
        .generate(request(&ws, &provider_id, "Write a task description"))
        .await?
        .into_inner()
        .text;
    assert_eq!(text, "Implement the widget.");

    Ok(())
}

#[tokio::test]
async fn generate_surfaces_a_failed_run_as_an_error() -> Result<()> {
    let recipe = serde_json::json!({
        "stream": [
            {"type": "system", "subtype": "init", "session_id": "gen-session"}
        ],
        "exit_code": 1
    })
    .to_string();
    let h = Harness::start_with_env(&[("MODULA_MOCK_RECIPE", &recipe)]).await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let provider_id = provider(&h, &ws).await?;

    let status = h
        .providers()
        .generate(request(&ws, &provider_id, "Write a task description"))
        .await
        .expect_err("expected an error status");
    assert_eq!(status.code(), Code::Internal);

    Ok(())
}

#[tokio::test]
async fn generate_rejects_an_empty_instruction() -> Result<()> {
    let h = Harness::start().await?;
    let ws = common::fresh_workspace(&h, "demo").await?;
    let provider_id = provider(&h, &ws).await?;

    let status = h
        .providers()
        .generate(request(&ws, &provider_id, "   "))
        .await
        .expect_err("expected an error status");
    assert_eq!(status.code(), Code::InvalidArgument);

    Ok(())
}
