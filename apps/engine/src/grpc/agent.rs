use modula_rpc::json::struct_to_json;
use modula_rpc::v1::{
    agent_service_server::AgentService, Agent, AgentArgDef, AgentSchedule, AgentSkill,
    CreateAgentRequest, CreateAgentResponse, DeleteAgentRequest, DeleteAgentResponse,
    GetAgentConfigRequest, GetAgentConfigResponse, GetAgentRequest, KillAgentRequest,
    KillAgentResponse, ListRunningAgentsRequest, ListRunningAgentsResponse, ListSkillsRequest,
    ListSkillsResponse, ListSystemToolsRequest, ListSystemToolsResponse, RunningAgent, SystemTool,
    TriggerAgentRequest, TriggerAgentResponse, UpdateAgentRequest, UpdateAgentResponse,
};
use serde_json::Value as JsonValue;
use tonic::{Request, Response, Status};

use crate::services::agents::{ArgInput, CreateParams, ScheduleParam, UpdateParams};
use crate::services::tools;
use crate::state::AppState;

use super::error::to_status;

pub struct AgentHandler {
    pub state: AppState,
}

fn arg_inputs(args: Vec<AgentArgDef>) -> Vec<ArgInput> {
    args.into_iter()
        .map(|a| ArgInput {
            flag: a.flag,
            required: a.required,
            help: a.help,
        })
        .collect()
}

fn schedule_param(s: AgentSchedule) -> ScheduleParam {
    ScheduleParam {
        cron: s.cron,
        timezone: s.timezone,
        enabled: s.enabled,
    }
}

#[tonic::async_trait]
impl AgentService for AgentHandler {
    async fn list_running(
        &self,
        req: Request<ListRunningAgentsRequest>,
    ) -> Result<Response<ListRunningAgentsResponse>, Status> {
        let ws = req.into_inner().workspace_id;
        let agents = self
            .state
            .agents
            .list_running(&ws)
            .await
            .into_iter()
            .map(RunningAgent::from)
            .collect();
        Ok(Response::new(ListRunningAgentsResponse { agents }))
    }

    async fn get(&self, req: Request<GetAgentRequest>) -> Result<Response<Agent>, Status> {
        let body = req.into_inner();
        let agent = self
            .state
            .agents
            .get(&body.workspace_id, &body.agent_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(agent.into()))
    }

    async fn get_config(
        &self,
        req: Request<GetAgentConfigRequest>,
    ) -> Result<Response<GetAgentConfigResponse>, Status> {
        let ws = req.into_inner().workspace_id;
        // The config list carries `next_fire` (filled by the service) but not the
        // prompt body.
        let agents = self
            .state
            .agents
            .list(&ws)
            .await
            .map_err(to_status)?
            .into_iter()
            .map(|a| Agent {
                prompt: None,
                ..a.into()
            })
            .collect();
        Ok(Response::new(GetAgentConfigResponse { agents }))
    }

    async fn list_skills(
        &self,
        req: Request<ListSkillsRequest>,
    ) -> Result<Response<ListSkillsResponse>, Status> {
        let ws = req.into_inner().workspace_id;
        let skills = self
            .state
            .agents
            .list_skills(&ws)
            .await
            .map_err(to_status)?
            .into_iter()
            .map(AgentSkill::from)
            .collect();
        Ok(Response::new(ListSkillsResponse { skills }))
    }

    async fn create(
        &self,
        req: Request<CreateAgentRequest>,
    ) -> Result<Response<CreateAgentResponse>, Status> {
        let body = req.into_inner();
        let created = self
            .state
            .agents
            .create(
                &body.workspace_id,
                CreateParams {
                    name: body.name,
                    description: body.description,
                    provider_id: body.provider_id,
                    model: body.model,
                    manual: body.manual,
                    schedule: body.schedule.map(schedule_param),
                    rules: body.rules,
                    args: arg_inputs(body.args),
                    prompt: Some(body.prompt),
                    spawn_per_variant: body.spawn_per_variant,
                    skills: body.skills,
                },
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(CreateAgentResponse {
            id: created.id,
            name: created.name,
        }))
    }

    async fn update(
        &self,
        req: Request<UpdateAgentRequest>,
    ) -> Result<Response<UpdateAgentResponse>, Status> {
        let body = req.into_inner();
        let model = if body.clear_model {
            Some(None)
        } else {
            body.model.map(Some)
        };
        let schedule = if body.clear_schedule {
            Some(None)
        } else {
            body.schedule.map(|s| Some(schedule_param(s)))
        };
        self.state
            .agents
            .update(
                &body.workspace_id,
                &body.agent_id,
                UpdateParams {
                    description: body.description,
                    provider_id: body.provider_id,
                    model,
                    manual: body.manual,
                    schedule,
                    rules: body.update_rules.then_some(body.rules),
                    args: body.update_args.then(|| arg_inputs(body.args)),
                    prompt: body.prompt,
                    spawn_per_variant: body.spawn_per_variant,
                    skills: body.update_skills.then_some(body.skills),
                },
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(UpdateAgentResponse { id: body.agent_id }))
    }

    async fn delete(
        &self,
        req: Request<DeleteAgentRequest>,
    ) -> Result<Response<DeleteAgentResponse>, Status> {
        let body = req.into_inner();
        self.state
            .agents
            .delete(&body.workspace_id, &body.agent_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(DeleteAgentResponse { id: body.agent_id }))
    }

    async fn trigger(
        &self,
        req: Request<TriggerAgentRequest>,
    ) -> Result<Response<TriggerAgentResponse>, Status> {
        let body = req.into_inner();
        let raw_args = body.args.map(struct_to_json).unwrap_or(JsonValue::Null);
        let res = self
            .state
            .agents
            .trigger(&body.workspace_id, &body.agent_id, raw_args)
            .await
            .map_err(to_status)?;
        Ok(Response::new(TriggerAgentResponse {
            id: res.id,
            name: res.name,
            pid: res.pid as i32,
            args: res.args,
        }))
    }

    async fn kill(
        &self,
        req: Request<KillAgentRequest>,
    ) -> Result<Response<KillAgentResponse>, Status> {
        let body = req.into_inner();
        let result = self
            .state
            .agents
            .kill(&body.workspace_id, body.pid, body.escalate)
            .await
            .map_err(to_status)?;
        Ok(Response::new(KillAgentResponse {
            ok: true,
            message: result["signal"].as_str().unwrap_or_default().to_string(),
            loop_cancelled: result["loop_cancelled"].as_bool().unwrap_or(false),
        }))
    }

    async fn list_system_tools(
        &self,
        _req: Request<ListSystemToolsRequest>,
    ) -> Result<Response<ListSystemToolsResponse>, Status> {
        let tools = tools::detect()
            .into_iter()
            .map(|(id, installed)| SystemTool {
                id: id.to_string(),
                installed,
            })
            .collect();
        Ok(Response::new(ListSystemToolsResponse { tools }))
    }
}
