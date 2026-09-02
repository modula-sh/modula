use modula_rpc::v1::{search_service_server::SearchService, SearchRequest, SearchResponse};
use tonic::{Request, Response, Status};

use modula_state::AppState;

pub struct SearchHandler {
    pub state: AppState,
}

#[tonic::async_trait]
impl SearchService for SearchHandler {
    async fn search(
        &self,
        req: Request<SearchRequest>,
    ) -> Result<Response<SearchResponse>, Status> {
        let body = req.into_inner();
        let hits = self
            .state
            .search
            .search(&body.workspace_id, &body.query, &body.kinds, body.limit)
            .await
            .map_err(super::error::to_status)?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Response::new(SearchResponse { hits }))
    }
}
