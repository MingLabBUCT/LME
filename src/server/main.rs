use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use clap::Parser;
use handlers::{
    add_layers, clone_stacks, create_layers, create_stack, create_workspace, export_workspace, get_layers, read_layer, read_stack, remove_unused_layers, remove_workspace, slice_stack
};
use lme::workspace::{LayerStorage, StackCache};
use middlewares::workspace_middleware;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::RwLock;

mod handlers;
mod middlewares;

pub type AppState = Arc<
    RwLock<
        BTreeMap<
            String,
            (
                Arc<RwLock<LayerStorage>>,
                Arc<RwLock<BTreeMap<String, Vec<usize>>>>,
                Arc<RwLock<StackCache>>,
            ),
        >,
    >,
>;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkspaceName {
    name: String,
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct ServerStartParameters {
    #[arg(short, long)]
    listen: String
}

#[tokio::main]
async fn main() {
    let start_parameters = ServerStartParameters::parse();
    let server_state: AppState = Default::default();
    let workspace_router = Router::new()
        .route("/stacks/new", post(create_stack))
        .route("/layers/new", post(create_layers))
        .route("/layers/remove_unused", put(remove_unused_layers))
        .route("/layers/:layer_id", get(read_layer))
        .route("/layers", get(get_layers))
        .route("/stacks/:stack_name", get(read_stack))
        .route("/stacks/:stack_name/clone", post(clone_stacks))
        .route("/stacks/:stack_name/slice", put(slice_stack))
        .route("/stacks/:stack_name/add", put(add_layers))
        .route("/export", get(export_workspace))
        .layer(middleware::from_fn_with_state(
            server_state.clone(),
            workspace_middleware,
        ));
    let app = Router::new()
        .nest("/workspace/:name", workspace_router)
        .route("/workspace", post(create_workspace))
        .route("/workspace/:name", delete(remove_workspace))
        .with_state(server_state);
    let listener = tokio::net::TcpListener::bind(start_parameters.listen).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
