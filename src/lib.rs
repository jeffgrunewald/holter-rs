use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, IntoMakeService},
    serve::WithGracefulShutdown,
    Router,
};
use metrics_exporter_prometheus::{BuildError, PrometheusBuilder, PrometheusHandle};
#[cfg(feature = "db")]
use sqlx::{Database, Executor, IntoArguments, Pool};
use std::{future::Future, net::SocketAddr};
use tokio::net::TcpListener;

#[cfg(feature = "api-metrics")]
pub mod metrics_middleware;

#[cfg(feature = "db")]
#[derive(Clone)]
pub struct HolterState<DB>
where
    DB: Database + Clone + Send + Sync + 'static,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
{
    db: Option<Pool<DB>>,
    metric_handle: PrometheusHandle,
}

#[cfg(not(feature = "db"))]
#[derive(Clone)]
pub struct HolterState {
    metric_handle: PrometheusHandle,
}

#[cfg(feature = "db")]
pub struct HolterServer<DB>
where
    DB: Database + Clone + Send + Sync + 'static,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
{
    listen_addr: SocketAddr,
    state: HolterState<DB>,
}

#[cfg(not(feature = "db"))]
pub struct HolterServer {
    listen_addr: SocketAddr,
    state: HolterState,
}

#[cfg(feature = "db")]
pub struct HolterServerBuilder<DB>
where
    DB: Database + Clone + Send + Sync + 'static,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
{
    listen_addr: SocketAddr,
    db: Option<Pool<DB>>,
}

#[cfg(not(feature = "db"))]
pub struct HolterServerBuilder {
    listen_addr: SocketAddr,
}

#[cfg(feature = "db")]
impl<DB> Default for HolterServerBuilder<DB>
where
    DB: Database + Clone + Send + Sync + 'static,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
{
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:9090".parse().unwrap(),
            db: None,
        }
    }
}

#[cfg(not(feature = "db"))]
impl Default for HolterServerBuilder {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:9090".parse().unwrap(),
        }
    }
}

#[cfg(feature = "db")]
impl<DB> HolterServerBuilder<DB>
where
    DB: Database + Clone + Send + Sync + 'static,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_listen_addr(mut self, listen_addr: SocketAddr) -> Self {
        self.listen_addr = listen_addr;
        self
    }

    pub fn add_db_connection(mut self, pool: Pool<DB>) -> Self {
        self.db = Some(pool);
        self
    }

    pub fn build(self) -> Result<HolterServer<DB>, BuildError> {
        let metric_handle = PrometheusBuilder::new().install_recorder()?;
        let state = HolterState {
            db: self.db,
            metric_handle,
        };
        Ok(HolterServer {
            listen_addr: self.listen_addr,
            state,
        })
    }
}

#[cfg(not(feature = "db"))]
impl HolterServerBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_listen_addr(mut self, listen_addr: SocketAddr) -> Self {
        self.listen_addr = listen_addr;
        self
    }

    pub fn build(self) -> Result<HolterServer, BuildError> {
        let metric_handle = PrometheusBuilder::new().install_recorder()?;
        let state = HolterState { metric_handle };
        Ok(HolterServer {
            listen_addr: self.listen_addr,
            state,
        })
    }
}

#[cfg(feature = "db")]
impl<DB> HolterServer<DB>
where
    DB: Database + Clone + Send + Sync + 'static,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
{
    pub async fn serve<L, M, S, F>(
        self,
        signal: F,
    ) -> WithGracefulShutdown<TcpListener, IntoMakeService<Router>, Router, F>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let router = Router::new()
            .route("/metrics", get(metrics_handler))
            .route("/healthz", get(health_handler))
            .with_state(self.state.clone());

        axum::serve(
            TcpListener::bind(&self.listen_addr)
                .await
                .expect("holter tcp bind error"),
            router.into_make_service(),
        )
        .with_graceful_shutdown(signal)
    }
}

#[cfg(not(feature = "db"))]
impl HolterServer {
    pub async fn serve<L, M, S, F>(
        self,
        signal: F,
    ) -> WithGracefulShutdown<TcpListener, IntoMakeService<Router>, Router, F>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let router = Router::new()
            .route("/metrics", get(metrics_handler))
            .route("/healthz", get(health_handler))
            .with_state(self.state.clone());

        axum::serve(
            TcpListener::bind(&self.listen_addr)
                .await
                .expect("holter tcp bind error"),
            router.into_make_service(),
        )
        .with_graceful_shutdown(signal)
    }
}

#[cfg(feature = "db")]
pub async fn health_handler<DB>(State(data): State<HolterState<DB>>) -> impl IntoResponse
where
    DB: Database + Clone + Send + Sync + 'static,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
{
    if let Some(ref db) = data.db {
        if sqlx::query("SELECT 1").execute(db).await.is_ok() {
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }
    StatusCode::OK
}

#[cfg(not(feature = "db"))]
pub async fn health_handler(_state: State<HolterState>) -> impl IntoResponse {
    StatusCode::OK
}

#[cfg(feature = "db")]
pub async fn metrics_handler<DB>(State(data): State<HolterState<DB>>) -> impl IntoResponse
where
    DB: Database + Clone + Send + Sync + 'static,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
{
    data.metric_handle.render()
}

#[cfg(not(feature = "db"))]
pub async fn metrics_handler(State(data): State<HolterState>) -> impl IntoResponse {
    data.metric_handle.render()
}
