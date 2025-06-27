# Metrics and Monitoring

ChronoNode Archival Client provides comprehensive metrics and monitoring capabilities through Prometheus. This document describes the available metrics and how to access them.

## Available Metrics

### Event Processing Metrics

- `chrononode_events_processed_total` - Counter of events processed, labeled by:
  - `event_type`: Type of the event (e.g., "block_mined", "transaction_received")
  - `status`: Status of processing ("success", "error", "published")

- `chrononode_event_processing_duration_seconds` - Histogram of event processing times, labeled by:
  - `event_type`: Type of the event

### Queue Metrics

- `chrononode_event_queue_size` - Gauge of current queue sizes, labeled by:
  - `queue`: Name of the queue (e.g., "event_bus")

### Error Metrics

- `chrononode_event_processing_errors_total` - Counter of processing errors, labeled by:
  - `error_type`: Type of error (e.g., "publish_failed", "handler_error")

## Accessing Metrics

Metrics are exposed via an HTTP endpoint that can be scraped by Prometheus.

### Default Configuration

By default, the metrics server runs on:

```
http://localhost:9090/metrics
```

### Health Check

A health check endpoint is available at:

```
http://localhost:9090/health
```

## Configuration

Metrics collection can be configured using the following environment variables:

- `METRICS_ENABLED`: Set to "true" to enable metrics collection (default: true)
- `METRICS_ADDRESS`: Address to bind the metrics server (default: "0.0.0.0:9090")

## Example Prometheus Configuration

Add the following to your `prometheus.yml` to scrape metrics from ChronoNode:

```yaml
scrape_configs:
  - job_name: 'chrononode'
    static_configs:
      - targets: ['localhost:9090']
```

## Example Queries

### Events Processed per Second

```promql
rate(chrononode_events_processed_total[5m])
```

### 99th Percentile Latency

```promql
histogram_quantile(0.99, sum(rate(chrononode_event_processing_duration_seconds_bucket[5m])) by (le, event_type))
```

### Error Rate

```promql
sum(rate(chrononode_event_processing_errors_total[5m])) by (error_type)
```

## Alerting

Example alert rules for common issues:

### High Error Rate

```yaml
- alert: HighErrorRate
  expr: rate(chrononode_event_processing_errors_total[5m]) > 0.1
  for: 5m
  labels:
    severity: critical
  annotations:
    summary: "High error rate detected"
    description: "Error rate is {{ $value }} errors per second"
```

### Growing Queue Size

```yaml
- alert: GrowingQueueSize
  expr: predict_linear(chrononode_event_queue_size[10m], 60 * 5) > 1000
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "Queue size is growing rapidly"
    description: "Queue is predicted to exceed 1000 items in 5 minutes"
```

## Grafana Dashboard

A sample Grafana dashboard is available in the `dashboards/` directory. Import this into your Grafana instance to visualize the metrics.
