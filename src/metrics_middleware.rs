use axum::{
    extract::{MatchedPath, Request},
    response::Response,
};
use futures::future::BoxFuture;
use std::{
    task::{Context, Poll},
    time::Instant,
};
use tower::{Layer, Service};

#[derive(Clone)]
pub struct MetricsLayer;

impl<S> Layer<S> for MetricsLayer {
    type Service = MetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MetricsService { inner }
    }
}

#[derive(Clone)]
pub struct MetricsService<S> {
    inner: S,
}

impl<S> Service<Request> for MetricsService<S>
where
    S: Service<Request, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    #[inline]
    fn poll_ready(&mut self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(ctx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let start = Instant::now();
        let monitored_path = req
            .extensions()
            .get::<MatchedPath>()
            .map(|path| path.as_str().to_owned());
        let method = req.method().clone();

        let mut inner = self.inner.clone();
        Box::pin(async move {
            let response = inner.call(req).await;

            if let Some(path) = monitored_path {
                let latency = start.elapsed().as_secs_f64();
                let status = if let Ok(ref response) = response {
                    response.status().as_u16()
                } else {
                    500
                };

                let labels = [
                    ("method", method.to_string()),
                    ("path", path),
                    ("status", status.to_string()),
                ];

                metrics::counter!("http-request-count", &labels).increment(1);
                metrics::histogram!("http-request-duration", &labels).record(latency);
            }

            response
        })
    }
}
