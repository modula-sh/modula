//! Integration config CRUD + proxied search/fetch against the external
//! service. `connect` health-checks the connection before persisting, so a
//! stored config is known-good at write time. Catalog CRUD carries no domain
//! event (labels precedent); the settings UI reads via plain queries.

use serde_json::Value as Json;
use sqlx::SqlitePool;

use modula_db::integrations::IntegrationRepository;
use modula_rpc::status::DomainError;
use modula_types::Integration;

type Result<T> = std::result::Result<T, DomainError>;

#[derive(Clone)]
pub struct IntegrationsService {
    pool: SqlitePool,
    integrations: IntegrationRepository,
}

impl IntegrationsService {
    pub fn new(pool: SqlitePool, integrations: IntegrationRepository) -> Self {
        Self { pool, integrations }
    }

    pub async fn list(&self, ws: &str) -> Result<Vec<Integration>> {
        self.integrations.list(&self.pool, ws).await
    }

    pub async fn connect(&self, ws: &str, id: &str, data: Json) -> Result<()> {
        let integration = modula_integrations::from_config(id, &data).map_err(bad_request)?;
        integration.health_check().await.map_err(bad_request)?;
        self.integrations.upsert(&self.pool, ws, id, &data).await
    }

    pub async fn delete(&self, ws: &str, id: &str) -> Result<()> {
        self.integrations.delete(&self.pool, ws, id).await
    }

    pub async fn search(
        &self,
        ws: &str,
        id: &str,
        query: &str,
        params: Json,
    ) -> Result<Vec<modula_integrations::ExternalItem>> {
        let integration = self.connected(ws, id, params).await?;
        integration.search(query).await.map_err(bad_request)
    }

    pub async fn fetch(
        &self,
        ws: &str,
        id: &str,
        key: &str,
        params: Json,
    ) -> Result<modula_integrations::ExternalItem> {
        let integration = self.connected(ws, id, params).await?;
        integration.fetch(key).await.map_err(bad_request)
    }

    /// Repositories for the import modal's repo dropdown; only github has any.
    pub async fn repos(&self, ws: &str, id: &str) -> Result<Vec<String>> {
        if id != "github" {
            return Err(DomainError::BadRequest(format!("{id} has no repositories")));
        }
        let row = self.row(ws, id).await?;
        modula_integrations::list_repos(&row.data).map_err(bad_request)
    }

    /// Build the client for a stored config with per-request `params` merged
    /// over it (e.g. github `repo`); `NotFound` when not connected.
    async fn connected(
        &self,
        ws: &str,
        id: &str,
        params: Json,
    ) -> Result<Box<dyn modula_integrations::Integration>> {
        let row = self.row(ws, id).await?;
        modula_integrations::from_config(id, &merge(row.data, params)).map_err(bad_request)
    }

    async fn row(&self, ws: &str, id: &str) -> Result<Integration> {
        self.integrations
            .get(&self.pool, ws, id)
            .await?
            .ok_or_else(|| DomainError::NotFound(format!("integration {id} is not connected")))
    }
}

fn merge(mut data: Json, params: Json) -> Json {
    if let (Some(obj), Some(extra)) = (data.as_object_mut(), params.as_object()) {
        obj.extend(extra.clone());
    }
    data
}

fn bad_request(e: modula_integrations::Error) -> DomainError {
    DomainError::BadRequest(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::testkit::env;
    use serde_json::json;

    fn service(env: &crate::services::testkit::Env) -> IntegrationsService {
        IntegrationsService::new(env.pool.clone(), IntegrationRepository::new())
    }

    #[tokio::test]
    async fn connect_rejects_unknown_id() {
        let env = env().await;
        let svc = service(&env);
        assert!(matches!(
            svc.connect(&env.ws, "s3", json!({})).await,
            Err(DomainError::BadRequest(_))
        ));
    }

    #[tokio::test]
    async fn connect_rejects_bad_config() {
        let env = env().await;
        let svc = service(&env);
        assert!(matches!(
            svc.connect(&env.ws, "jira", json!({})).await,
            Err(DomainError::BadRequest(_))
        ));
    }

    #[tokio::test]
    async fn search_requires_connected_integration() {
        let env = env().await;
        let svc = service(&env);
        assert!(matches!(
            svc.search(&env.ws, "linear", "q", json!({})).await,
            Err(DomainError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn repos_requires_connected_github() {
        let env = env().await;
        let svc = service(&env);
        assert!(matches!(
            svc.repos(&env.ws, "github").await,
            Err(DomainError::NotFound(_))
        ));
        assert!(matches!(
            svc.repos(&env.ws, "linear").await,
            Err(DomainError::BadRequest(_))
        ));
    }

    #[test]
    fn merge_overlays_params_on_stored_data() {
        let merged = merge(
            json!({"use_gh_cli": true, "repo": "old/stale"}),
            json!({"repo": "acme/site"}),
        );
        assert_eq!(merged, json!({"use_gh_cli": true, "repo": "acme/site"}));
        assert_eq!(merge(json!({"a": 1}), json!({})), json!({"a": 1}));
    }

    // Connect health-checks against the live service, so persistence is
    // seeded through the repository here.
    #[tokio::test]
    async fn list_and_delete_round_trip() {
        let env = env().await;
        let svc = service(&env);
        let repo = IntegrationRepository::new();
        let data = json!({"api_token": "lin_api_x"});
        repo.upsert(&env.pool, &env.ws, "linear", &data)
            .await
            .unwrap();

        let listed = svc.list(&env.ws).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "linear");
        assert_eq!(listed[0].data, data);

        svc.delete(&env.ws, "linear").await.unwrap();
        assert!(svc.list(&env.ws).await.unwrap().is_empty());
        assert!(matches!(
            svc.delete(&env.ws, "linear").await,
            Err(DomainError::NotFound(_))
        ));
    }
}
