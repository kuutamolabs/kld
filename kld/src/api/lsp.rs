use std::sync::Arc;

use api::LspListProtocolsParams;
use axum::{extract::Query, response::IntoResponse, Extension, Json};

use crate::ldk::LightningInterface;

use super::{internal_server, ApiError};

pub(crate) async fn list_protocols(
    Extension(lightning_interface): Extension<Arc<dyn LightningInterface + Send + Sync>>,
    Query(params): Query<LspListProtocolsParams>,
) -> Result<impl IntoResponse, ApiError> {
    let protocols: Vec<String> = lightning_interface
        .lsp_list_protocols(params.node_id)
        .await
        .map_err(internal_server)?;
    Ok(Json(protocols))
}
