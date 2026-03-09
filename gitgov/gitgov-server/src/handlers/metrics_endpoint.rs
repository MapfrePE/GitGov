// Prometheus metrics endpoint.
// Exposed at `/metrics` without authentication for scraper compatibility.
//
// Note: Extension, IntoResponse, etc. are already in scope via include!().

pub async fn prometheus_metrics(
    axum::Extension(handle): axum::Extension<metrics_exporter_prometheus::PrometheusHandle>,
) -> String {
    handle.render()
}
