# Tracing and Metrics Design for Rhythm

> **Status**: Planning / Design Phase
> **Last Updated**: 2025-10-11

## Overview

This document outlines the design considerations for adding distributed tracing and metrics to Rhythm. Given Rhythm's unique architecture (Rust core + language adapters + PostgreSQL-only coordination), observability needs careful thought around:

1. Where instrumentation happens (Rust core vs language adapters)
2. How trace context propagates through the database
3. Supporting multiple backends (OpenTelemetry, Datadog, etc.)
4. Keeping it zero-cost when disabled
5. Extensibility for language-specific integrations

---

## Architecture Layers to Instrument

### 1. Rust Core (`/core`)
- Database operations (claim, report, create)
- Worker coordination (heartbeats, failover)
- Execution state transitions
- Critical performance paths

### 2. Language Adapters (`/python`, `/node`)
- Function invocation
- Workflow replay mechanics
- Serialization/deserialization
- FFI boundary crossings

### 3. User Code
- Task/task/workflow execution
- Custom instrumentation via context API

---

## Tracing Design

### Core Tracing Implementation

**Proposal: Rust core uses `tracing` crate with OTLP export**

**Rationale:**
- `tracing` is the de facto standard in Rust ecosystem
- OTLP (OpenTelemetry Protocol) is universal format
- Works with: Datadog (via agent), Honeycomb, Jaeger, Grafana Tempo, etc.
- Keeps core opinionated but flexible

**Alternative Considered:**
- Core defines trait/callback interface, language adapters inject implementation
- **Rejected**: Too complex, most users want OTLP anyway

### Cross-Language Trace Propagation

**Key Challenge**: Maintain trace context across execution boundaries:
```
Python Workflow → Rust Core → PostgreSQL → Rust Core → Python Task
```

**Proposed Solution: Database as Propagation Medium**

Add columns to `executions` table:
- `trace_id` (VARCHAR) - W3C Trace Context format, propagated parent → child
- `span_id` (VARCHAR) - Unique per execution
- `trace_context` (JSONB) - Optional baggage/custom attributes

**Propagation Flow:**
1. Initial task/workflow receives `trace_id` (from caller or generated)
2. Core stores `trace_id` in execution record
3. When creating child executions (Tasks), inherit parent's `trace_id`
4. Workers pick up executions, read `trace_id`, continue trace in language runtime

**Benefits:**
- ✅ Database already coordinates execution tree
- ✅ Works across worker restarts/failovers
- ✅ Standard W3C Trace Context format
- ✅ Language-agnostic at database layer

### Language Adapter Integration

**Each language adapter can hook into native tracing libraries:**

**Python:**
- OpenTelemetry SDK (`opentelemetry-api`, `opentelemetry-sdk`)
- Datadog `ddtrace` (native integration)
- AWS X-Ray

**Node.js:**
- OpenTelemetry JS (`@opentelemetry/api`, `@opentelemetry/sdk-node`)
- Datadog APM
- New Relic

**Adapter Responsibilities:**
1. Read `trace_id`/`span_id` from execution metadata
2. Create span in native library (OpenTelemetry, Datadog, etc.)
3. Wrap user function execution with span
4. Report span completion back to core (optional)

**Hybrid Model (Recommended):**
- **Core handles**: Low-level spans (DB queries, worker operations)
- **Adapters handle**: User-code spans (function execution, workflow steps)
- **Both emit**: To same `trace_id` for unified trace

### Configuration

**Option 1: Environment Variables (Simple)**
```bash
RHYTHM_TRACING_ENABLED=true
RHYTHM_TRACING_ENDPOINT=http://localhost:4317  # OTLP endpoint
RHYTHM_TRACING_SERVICE_NAME=my-app
RHYTHM_TRACES_SAMPLE_RATE=0.1
RHYTHM_TRACES_SAMPLE_ERRORS=true  # Always sample failures
```

**Option 2: Per-Worker Config (Flexible)**
```python
worker = Worker(
    observability={
        "traces": {
            "enabled": True,
            "endpoint": "http://localhost:4317",
            "sample_rate": 0.1,
            "sample_errors": True,
        },
        "service_name": "my-worker",
        "tags": {"env": "production", "version": "1.2.3"}
    }
)
```

**Option 3: Hybrid (Recommended)**
- Core provides sensible defaults via env vars
- Language adapters can override programmatically
- Per-worker config takes precedence

### What Gets Traced?

**Core Operations (Rust):**
- `db.claim_execution`
  - Attributes: queue_name, worker_id, execution_type
  - Duration, success/failure
- `db.report_result`
  - Attributes: execution_id, status, result_size
- `db.create_execution`
  - Attributes: parent_id, execution_type, queue
- `worker.heartbeat`
  - Attributes: worker_id
- `worker.detect_dead_workers`
  - Attributes: dead_worker_ids[], reassigned_count

**Adapter Operations (Python/Node):**
- `function.invoke`
  - Attributes: function_name, execution_type
  - Arguments (sanitized/hashed for privacy)
  - Duration, success/failure
- `workflow.replay`
  - Attributes: workflow_name, replay_count, checkpoint_size
- `serialization`
  - Attributes: payload_size, format (json/pickle)

**User Code (Automatic):**
- Span per task/task/workflow execution
- Name: `{execution_type}.{function_name}`
- Users can add custom spans via context API (future)

### Extensibility for Different Backends

**Core: OTLP Only**
- OpenTelemetry Protocol is the universal format
- Works with all major backends (via agents or direct ingestion)
- Keeps Rust core simple and focused

**Language Adapters: Vendor-Specific Integrations**
- Python can add native `ddtrace` support
- Node.js can add Datadog APM native support
- Users opt into vendor features if needed

**Example: Python with Datadog**
```python
# Core still exports OTLP
# But adapter can also use ddtrace for automatic instrumentation
worker = Worker(
    observability={
        "traces": {
            "provider": "datadog",  # Uses ddtrace under the hood
            "service_name": "my-service",
        }
    }
)
```

---

## Metrics Design

### Why Metrics Matter

Metrics are arguably **more important** than traces for operational monitoring:
- Cheaper than traces (aggregated in-process)
- Essential for alerting (queue depth, error rate, latency)
- Better for dashboards and SLOs

### Core Metrics (Rust)

**Counters:**
- `rhythm.executions.claimed`
  - Labels: `queue`, `worker_id`, `execution_type`
- `rhythm.executions.completed`
  - Labels: `queue`, `status` (success/failed/cancelled), `execution_type`
- `rhythm.executions.created`
  - Labels: `queue`, `execution_type`, `has_parent` (bool)
- `rhythm.workflow.replays`
  - Labels: `workflow_name`

**Histograms:**
- `rhythm.execution.duration`
  - Labels: `execution_type`, `queue`, `status`
  - Measures: created_at → completed_at
- `rhythm.claim_loop.duration`
  - Labels: `worker_id`, `queue`
  - Measures: Time to claim next execution
- `rhythm.db.query.duration`
  - Labels: `operation` (claim/report/create/heartbeat)
  - Measures: Database query latency

**Gauges:**
- `rhythm.workers.active`
  - From `worker_heartbeats` table
  - Labels: None (or `queue` if workers subscribe to specific queues)
- `rhythm.executions.waiting`
  - Labels: `queue`, `execution_type`
  - Queue depth (pending executions)
- `rhythm.executions.running`
  - Labels: `queue`, `execution_type`, `worker_id`
  - Currently executing

### Adapter Metrics (Python/Node)

**Histograms:**
- `rhythm.function.execution.duration`
  - Labels: `function_name`, `execution_type`, `status`
  - Measures: User function execution time
- `rhythm.serialization.duration`
  - Labels: `direction` (serialize/deserialize), `format`

**Counters:**
- `rhythm.ffi.calls`
  - Labels: `function` (claim_execution/report_result/etc.)
  - Tracks FFI boundary crossings (for debugging overhead)

### Metrics Export Strategy

**Per-Worker Export (Recommended)**
- Each worker runs its own metrics exporter
- Push-based via OTLP
- Backends aggregate across workers

**Why not centralized collector?**
- ❌ Adds complexity (separate process)
- ❌ Requires workers to write metrics to DB (latency, extra load)
- ❌ Single point of failure

**OTLP vs Prometheus:**
- **OTLP (Push)**: Works with Prometheus (via OTLP receiver), Datadog, CloudWatch
- **Prometheus (Pull)**: Need to expose endpoint per worker (awkward service discovery)
- **Recommendation**: OTLP for simplicity, works everywhere

### Combined Configuration Example

**Environment Variables:**
```bash
# Tracing
RHYTHM_TRACING_ENABLED=true
RHYTHM_TRACING_ENDPOINT=http://localhost:4317
RHYTHM_TRACES_SAMPLE_RATE=0.1
RHYTHM_TRACES_SAMPLE_ERRORS=true

# Metrics
RHYTHM_METRICS_ENABLED=true
RHYTHM_METRICS_ENDPOINT=http://localhost:4317
RHYTHM_METRICS_INTERVAL=10  # Export every 10 seconds

# Common
RHYTHM_SERVICE_NAME=my-worker
RHYTHM_OBSERVABILITY_TAGS=env:production,version:1.2.3
```

**Programmatic Config:**
```python
worker = Worker(
    observability={
        "traces": {
            "enabled": True,
            "endpoint": "http://localhost:4317",
            "sample_rate": 0.1,
            "sample_errors": True,
        },
        "metrics": {
            "enabled": True,
            "endpoint": "http://localhost:4317",
            "interval": 10,  # seconds
        },
        "service_name": "my-worker",
        "tags": {"env": "production", "version": "1.2.3"}
    }
)
```

---

## Open Questions

### 1. Sampling Strategy

**Question**: Who controls sampling decisions?

**Options:**
- **Head-based sampling in core**: Configured per worker (`sample_rate: 0.1`)
- **Tail-based sampling in backend**: Datadog/Honeycomb decide after seeing full trace
- **Always sample failures**: Critical for debugging, regardless of sample_rate

**Decision Needed**: Support all three? Make configurable?

### 2. Trace Baggage

**Question**: Do we need W3C baggage propagation?

**Context**: In HTTP-based systems, baggage carries metadata (tenant_id, request_id) across service boundaries. But Rhythm's execution tree is coordinated via database, not HTTP.

**Potential Use Cases:**
- Tenant/User ID: Filter traces by customer
- Request ID: Correlate with originating API request
- Feature flags: "This execution is part of experiment X"
- Authorization context: Original user permissions (security risk?)

**Options:**
- **No baggage**: `trace_id` gives execution tree, users can put custom data in `input` JSON
- **Simple tags**: `tags: Map<String, String>` in execution metadata (not full W3C spec)
- **Full W3C baggage**: Store in `trace_context` JSONB column

**Decision Needed**: Start without it? Add simple tags? Full spec?

### 3. Zero-Cost When Disabled

**Question**: How to ensure no overhead when tracing/metrics disabled?

**Rust Core:**
- ✅ `tracing` crate has compile-time filtering (zero-cost when disabled)
- ✅ Conditional compilation with feature flags

**FFI Boundary:**
- ⚠️ If language adapters call `rust_bridge.start_span()` every execution, that's FFI overhead even when disabled
- **Solution 1**: Check enabled flag in adapter before FFI call
  ```python
  if self._tracing_enabled:
      RustBridge.start_span(...)
  ```
- **Solution 2**: Batch span events, only send to core when flushing
- **Solution 3**: Core returns no-op span handle when disabled

**Decision Needed**: Which approach? Benchmark overhead?

### 4. Metrics vs Traces Priority

**Question**: Ship both at once, or metrics first?

**Arguments for metrics first:**
- More operationally critical (alerts, dashboards)
- Cheaper than traces
- Easier to implement (no cross-execution propagation)

**Arguments for traces first:**
- Better debugging experience
- Already designed trace propagation via DB
- Metrics can piggyback on same OTLP infrastructure

**Arguments for both together:**
- Same export infrastructure (OTLP endpoint)
- Users expect both from modern systems
- Implementation overlap (configuration, core integration)

**Decision Needed**: Phased rollout or together?

### 5. Prometheus Scraping Support

**Question**: Support Prometheus pull-based metrics?

**Challenges:**
- Workers are ephemeral, need service discovery
- Which port does each worker expose metrics on?
- Kubernetes: Need sidecar or pod annotations

**Options:**
- **OTLP only**: Simpler, works with Prometheus via OTLP receiver
- **Optional Prometheus exporter**: Workers expose `:9090/metrics` endpoint
- **Sidecar pattern**: Separate process collects metrics, exposes Prometheus endpoint

**Decision Needed**: OTLP-only for v1? Add Prometheus later if requested?

### 6. Custom Instrumentation API

**Question**: Should users be able to add custom spans/metrics in their code?

**Example:**
```python
@task()
async def process_order(order_id: str):
    with rhythm.trace.span("validate_order"):
        # custom logic
        pass

    rhythm.metrics.increment("orders.processed", tags={"status": "success"})
```

**Considerations:**
- Requires exposing tracing/metrics API in context
- Different APIs per language (OpenTelemetry Python vs Node.js)
- Or: Rhythm provides unified API that adapts to backend

**Decision Needed**: Ship custom instrumentation in v1? Or automatic-only initially?

### 7. Performance Impact on Claim Loop

**Question**: Will tracing/metrics slow down the hot path (claim loop)?

**Critical Path**: `claim_execution()` runs every 50-100ms per worker. Adding spans/metrics could:
- Increase DB payload size (`trace_id` in WHERE clauses?)
- Add FFI overhead (reporting spans)
- Allocate memory for span context

**Mitigation:**
- Benchmark with tracing enabled vs disabled
- Use sampling to reduce overhead
- Batch metric exports (not per-execution)
- Keep `trace_id` indexed for fast queries

**Decision Needed**: Set performance budget? Benchmark before merging?

### 8. Multi-Tenant Tracing

**Question**: How to isolate traces in multi-tenant deployments?

**Scenario**: Single Rhythm deployment serves multiple customers/teams.

**Options:**
- **Service name per tenant**: `service_name: "rhythm-tenant-123"`
- **Tag-based filtering**: `tags: {tenant_id: "123"}`
- **Separate OTLP endpoints**: Route tenant A to endpoint X, tenant B to endpoint Y

**Decision Needed**: Recommend pattern? Provide built-in support?

---

## Implementation Phases (Proposed)

### Phase 1: Core Metrics (Foundation)
- Add `metrics` crate to Rust core
- Implement basic counters/histograms (executions.claimed, execution.duration)
- OTLP export via environment variables
- Per-worker export
- **Goal**: Operational visibility into worker health

### Phase 2: Database Schema for Traces
- Add `trace_id`, `span_id` columns to `executions` table
- Propagate trace context parent → child
- Migration script
- **Goal**: Infrastructure for distributed tracing

### Phase 3: Core Tracing
- Add `tracing` crate to Rust core
- Instrument DB operations, worker coordination
- OTLP export (same endpoint as metrics)
- Read/write trace context from `executions` table
- **Goal**: Visibility into core operations

### Phase 4: Language Adapter Tracing
- Python: OpenTelemetry SDK integration
- Node.js: OpenTelemetry JS integration
- Read trace context from core, create spans for user functions
- **Goal**: End-to-end traces across Rust + Python/Node

### Phase 5: Custom Instrumentation API
- Expose `rhythm.trace.span()` and `rhythm.metrics.*` in user code
- Documentation and examples
- **Goal**: Users can add domain-specific observability

### Phase 6: Advanced Features
- Vendor-specific integrations (Datadog native, AWS X-Ray)
- Prometheus scraping support (if requested)
- Trace baggage (if use cases emerge)
- Performance optimizations based on benchmarks

---

## Success Criteria

**Metrics:**
- [ ] Queue depth visible in real-time
- [ ] Worker health (active workers, claim rate) trackable
- [ ] Execution latency (P50, P95, P99) measurable
- [ ] Error rate alerts possible

**Tracing:**
- [ ] End-to-end trace from workflow → Tasks
- [ ] Trace survives worker restarts/failovers
- [ ] Cross-language traces (Python workflow → Node task)
- [ ] Failed executions automatically sampled

**Performance:**
- [ ] <5% overhead on claim loop when enabled
- [ ] Zero overhead when disabled
- [ ] Metrics export <100ms per batch

**Usability:**
- [ ] Works with OTLP-compatible backends (Datadog, Honeycomb, Jaeger)
- [ ] Single configuration block for all observability
- [ ] Automatic traces for all executions (no manual instrumentation required)

---

## References

- [OpenTelemetry Specification](https://opentelemetry.io/docs/specs/otel/)
- [W3C Trace Context](https://www.w3.org/TR/trace-context/)
- [Rust `tracing` crate](https://docs.rs/tracing/)
- [Rust `metrics` crate](https://docs.rs/metrics/)
- [OTLP Protocol](https://opentelemetry.io/docs/specs/otlp/)

---

**Next Steps**: Review this design, answer open questions, then implement Phase 1 (Core Metrics).
