# Portable monitoring package for Rust servers, exposing metric and healthcheck endpoints

## What

Holter is intended to be a simple implementation of web-based monitoring metrics scraping
and healthcheck endpoints. It creates a small Axum webserver with a `/metrics` and a `/healthz`
endpoint for exporing Prometheus metrics for scraping (collected in rust by the `metrics` crate)
as well as a healthcheck endpoint for monitoring availability and health behind automated infra
solutions such as Kubernetes container orchestration (when wrapped in a container HEALTHCHECK) or
when fronted by an application load balancer.

## How

By default, the server listens on the local `127.0.0.1` address at the default Prometheus port
`9090` but this can be configured by passing it a `std::netSocketAddr` at startup. The service can
also be configured, via the `db` feature to monitor the ability to reach any database supported by
`sqlx` by pinging the database on each healthcheck endpoint request.

## Features

- `default`: bare bones metrics and healthz rest endpoints
- `db`: monitoring of a sqlx database connection
- `api-metrics`: provides a tower middleware that, when injected into an axum router, measures
and returns request/response latency histograms and invocation counts on all endpoints served by the router.
