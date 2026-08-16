use modula_rpc::json::json_to_struct;
use modula_rpc::v1::{
    CreateAgentRequest, DeleteAgentRequest, GetAgentConfigRequest, GetAgentRequest,
    KillAgentRequest, ListRunningAgentsRequest, ListSkillsRequest, ListSystemToolsRequest,
    TriggerAgentRequest, UpdateAgentRequest,
};
use modula_types::{Agent, AgentSkill, RunningAgent, SystemTool};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{rpc, ClientError};
use crate::request::WriteAgent;
use crate::ModulaClient;

/// Result of `create_agent` — the new agent's id and name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedAgent {
    pub id: String,
    pub name: String,
}

/// Result of `trigger_agent` — the spawned run's identity and resolved args.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggeredAgent {
    pub id: String,
    pub name: String,
    pub pid: i32,
    pub args: Vec<String>,
}

/// Result of `kill_agent` — whether the signal landed and a human message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillOutcome {
    pub ok: bool,
    pub message: String,
}

impl ModulaClient {
    pub async fn list_running_agents(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<RunningAgent>, ClientError> {
        let resp = self
            .agents()
            .await?
            .list_running(ListRunningAgentsRequest {
                workspace_id: workspace_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.agents.into_iter().map(RunningAgent::from).collect())
    }

    pub async fn get_agent(
        &self,
        workspace_id: &str,
        agent_id: &str,
    ) -> Result<Agent, ClientError> {
        let resp = self
            .agents()
            .await?
            .get(GetAgentRequest {
                workspace_id: workspace_id.to_string(),
                agent_id: agent_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(Agent::from(resp))
    }

    pub async fn agent_config(&self, workspace_id: &str) -> Result<Vec<Agent>, ClientError> {
        let resp = self
            .agents()
            .await?
            .get_config(GetAgentConfigRequest {
                workspace_id: workspace_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.agents.into_iter().map(Agent::from).collect())
    }

    pub async fn list_skills(&self, workspace_id: &str) -> Result<Vec<AgentSkill>, ClientError> {
        let resp = self
            .agents()
            .await?
            .list_skills(ListSkillsRequest {
                workspace_id: workspace_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.skills.into_iter().map(AgentSkill::from).collect())
    }

    pub async fn create_agent(
        &self,
        workspace_id: &str,
        body: WriteAgent,
    ) -> Result<CreatedAgent, ClientError> {
        let resp = self
            .agents()
            .await?
            .create(CreateAgentRequest {
                workspace_id: workspace_id.to_string(),
                name: body.name.unwrap_or_default(),
                description: body.description,
                provider_id: body.provider_id,
                model: body.model,
                manual: body.manual,
                schedule: body.schedule.map(Into::into),
                rules: body.rules,
                args: body.args.into_iter().map(Into::into).collect(),
                prompt: body.prompt,
                spawn_per_variant: body.spawn_per_variant,
                skills: body.skills,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(CreatedAgent {
            id: resp.id,
            name: resp.name,
        })
    }

    /// Edit an agent; returns the agent id the engine confirmed. An absent
    /// `model`/`schedule` clears it; rules/args/skills always replace.
    pub async fn update_agent(
        &self,
        workspace_id: &str,
        agent_id: &str,
        body: WriteAgent,
    ) -> Result<String, ClientError> {
        let clear_model = body.model.is_none();
        let clear_schedule = body.schedule.is_none();
        let resp = self
            .agents()
            .await?
            .update(UpdateAgentRequest {
                workspace_id: workspace_id.to_string(),
                agent_id: agent_id.to_string(),
                description: Some(body.description),
                provider_id: Some(body.provider_id),
                model: body.model,
                clear_model,
                manual: Some(body.manual),
                schedule: body.schedule.map(Into::into),
                clear_schedule,
                rules: body.rules,
                update_rules: true,
                args: body.args.into_iter().map(Into::into).collect(),
                update_args: true,
                prompt: Some(body.prompt),
                spawn_per_variant: Some(body.spawn_per_variant),
                skills: body.skills,
                update_skills: true,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.id)
    }

    pub async fn delete_agent(
        &self,
        workspace_id: &str,
        agent_id: &str,
    ) -> Result<(), ClientError> {
        self.agents()
            .await?
            .delete(DeleteAgentRequest {
                workspace_id: workspace_id.to_string(),
                agent_id: agent_id.to_string(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    pub async fn trigger_agent(
        &self,
        workspace_id: &str,
        agent_id: &str,
        args: Option<Value>,
    ) -> Result<TriggeredAgent, ClientError> {
        let resp = self
            .agents()
            .await?
            .trigger(TriggerAgentRequest {
                workspace_id: workspace_id.to_string(),
                agent_id: agent_id.to_string(),
                args: args.and_then(json_to_struct),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(TriggeredAgent {
            id: resp.id,
            name: resp.name,
            pid: resp.pid,
            args: resp.args,
        })
    }

    pub async fn kill_agent(
        &self,
        workspace_id: &str,
        pid: i32,
        escalate: bool,
    ) -> Result<KillOutcome, ClientError> {
        let resp = self
            .agents()
            .await?
            .kill(KillAgentRequest {
                workspace_id: workspace_id.to_string(),
                pid,
                escalate,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(KillOutcome {
            ok: resp.ok,
            message: resp.message,
        })
    }

    pub async fn list_system_tools(&self) -> Result<Vec<SystemTool>, ClientError> {
        let resp = self
            .agents()
            .await?
            .list_system_tools(ListSystemToolsRequest {})
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.tools.into_iter().map(SystemTool::from).collect())
    }
}
