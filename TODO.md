# Currant - TODO List

> **Last Updated**: 2025-10-11
> **Status**: Active development - Python adapter mature, Node.js in progress

This document tracks missing functionality and planned features for Currant. Items are prioritized based on recent architectural decisions and project maturity needs.

---

## **Priority 1: CLI Architecture & Configuration (Critical)**

These items reflect recent architectural decisions (2025-10-11) that need implementation.

### CLI Restructuring

**1. Global admin CLI binary** (`currant-cli`)
- [ ] Create separate cargo package for optional global installation
- [ ] Include commands: `list`, `status`, `cancel`, `signal`, `retry`, `cleanup`
- [ ] Exclude: `migrate`, `worker`, `bench` (language-specific)
- [ ] Update CONTEXT.md if architecture changes

**2. Remove `migrate` from core CLI**
- [ ] Remove `migrate` command from `core/src/cli.rs` (currently lines 150-154)
- [ ] Keep in Python adapter (`python/currant/__main__.py`)
- [ ] Add to Node adapter when implemented
- [ ] Update documentation

**3. Remove `bench` from core CLI**
- [ ] Remove `bench` command from `core/src/cli.rs` (currently lines 291-321)
- [ ] Move benchmarking to Python adapter CLI
- [ ] Move benchmarking to Node adapter when implemented
- [ ] Update CONTEXT.md to reflect adapter-specific benchmarking

**4. Implement `retry` command**
- [ ] Add to global admin CLI
- [ ] Implement in `core/src/executions.rs`
- [ ] Add CLI command definition in `core/src/cli.rs`
- [ ] Should retry failed executions
- [ ] Tests for retry functionality

**5. Implement `cleanup` command**
- [ ] Add to global admin CLI
- [ ] Implement in `core/src/executions.rs`
- [ ] Add CLI command definition in `core/src/cli.rs`
- [ ] Should purge old completed/failed executions
- [ ] Support options: `--older-than`, `--status`, `--queue`
- [ ] Tests for cleanup functionality

### Configuration Management

**6. `currant.toml` configuration file support**
- [ ] Add `toml` crate to core dependencies
- [ ] Implement config loading in Rust core
- [ ] Search path: `currant.toml` → `~/.config/currant/config.toml`
- [ ] Priority chain: CLI flag → env var → config file → fallback
- [ ] Optional `.env` file loading
- [ ] Tests for config loading and priority

**7. Config structure definition**
```toml
# Example structure to implement
[database]
url = "postgresql://..."

[observability]
enabled = true
endpoint = "http://localhost:4317"
service_name = "my-service"

[observability.traces]
sample_rate = 0.1
sample_errors = true

[observability.metrics]
interval = 10
```

### Versioning & Schema

**8. Schema version table/metadata**
- [ ] Add `currant_metadata` table with key-value storage
- [ ] Store schema version on each migration
- [ ] Add `get_schema_version()` function in core
- [ ] Version checking on CLI commands (warn on mismatch)
- [ ] Migration to create metadata table
- [ ] Document version compatibility strategy

---

## **Priority 2: Observability System (Metrics & Tracing)**

Complete observability implementation as designed in TRACING_DESIGN.md.

### Database Schema for Tracing

**9. Add tracing columns to executions table**
- [ ] Migration to add columns:
  - `trace_id` (VARCHAR) - W3C Trace Context format
  - `span_id` (VARCHAR) - Unique per execution
  - `trace_context` (JSONB) - Optional baggage
- [ ] Update `core/src/types.rs` Execution struct
- [ ] Propagate trace_id from parent to child executions
- [ ] Index on trace_id for efficient queries

### Core Metrics (Rust)

**10. Metrics infrastructure in core**
- [ ] Add `metrics` and `opentelemetry` crates
- [ ] OTLP exporter setup
- [ ] Configuration via env vars / config file
- [ ] Per-worker metric export
- [ ] Zero-cost when disabled (feature flag or runtime check)

**11. Implement core counters**
- [ ] `currant.executions.claimed` (labels: queue, worker_id, execution_type)
- [ ] `currant.executions.completed` (labels: queue, status, execution_type)
- [ ] `currant.executions.created` (labels: queue, execution_type, has_parent)
- [ ] `currant.workflow.replays` (labels: workflow_name)

**12. Implement core histograms**
- [ ] `currant.execution.duration` (labels: execution_type, queue, status)
- [ ] `currant.claim_loop.duration` (labels: worker_id, queue)
- [ ] `currant.db.query.duration` (labels: operation)

**13. Implement core gauges**
- [ ] `currant.workers.active` (from worker_heartbeats table)
- [ ] `currant.executions.waiting` (labels: queue, execution_type)
- [ ] `currant.executions.running` (labels: queue, execution_type, worker_id)

### Core Tracing (Rust)

**14. Tracing infrastructure in core**
- [ ] Add `tracing` and `tracing-subscriber` crates
- [ ] OTLP exporter for traces
- [ ] Instrument DB operations (claim, report, create)
- [ ] Instrument worker coordination (heartbeat, failover)
- [ ] Configuration via env vars / config file
- [ ] Zero-cost when disabled

**15. Trace context propagation**
- [ ] Read trace_id when creating child executions
- [ ] Write trace_id to child execution records
- [ ] Generate span_id per execution
- [ ] Expose trace context to language adapters via FFI

### Language Adapter Observability

**16. Python OpenTelemetry integration**
- [ ] Add OpenTelemetry SDK dependencies
- [ ] Read trace context from core (trace_id, span_id)
- [ ] Create spans for user function execution
- [ ] Implement adapter-specific metrics:
  - `currant.function.execution.duration`
  - `currant.serialization.duration`
  - `currant.ffi.calls`
- [ ] Configuration via Worker constructor
- [ ] Documentation and examples

**17. Node.js OpenTelemetry integration**
- [ ] Add OpenTelemetry SDK dependencies
- [ ] Similar implementation to Python
- [ ] OTLP export configuration
- [ ] Documentation and examples

**18. Observability configuration API**
- [ ] Support env vars:
  - `CURRANT_TRACING_ENABLED`
  - `CURRANT_TRACING_ENDPOINT`
  - `CURRANT_TRACES_SAMPLE_RATE`
  - `CURRANT_METRICS_ENABLED`
  - `CURRANT_METRICS_ENDPOINT`
  - `CURRANT_SERVICE_NAME`
- [ ] Support programmatic config in Worker constructor
- [ ] Support `currant.toml` observability section
- [ ] Validate config and provide helpful errors

### Advanced Observability (Optional)

**19. Custom instrumentation API**
- [ ] Expose span creation API to user code
- [ ] Expose metrics API to user code
- [ ] Python: `currant.trace.span()`, `currant.metrics.increment()`
- [ ] Node: Similar API
- [ ] Documentation with examples

**20. Vendor-specific integrations** (Optional)
- [ ] Datadog native integration (ddtrace)
- [ ] AWS X-Ray integration
- [ ] Document when to use vs OTLP

**21. Prometheus scraping support** (Optional)
- [ ] Expose metrics endpoint per worker
- [ ] Service discovery considerations
- [ ] Document Kubernetes setup

**22. Trace baggage** (If needed)
- [ ] Full W3C baggage implementation
- [ ] Store in trace_context JSONB column
- [ ] Use cases: tenant_id, request_id, feature flags

---

## **Priority 3: Node.js Adapter Maturity**

Bring Node.js adapter to parity with Python.

**23. Node.js CLI implementation**
- [ ] Create CLI entry point (`npx currant <command>`)
- [ ] Implement `worker` command handler
- [ ] Implement `bench` command handler (when bench moved from core)
- [ ] Implement `migrate` command handler (when migrate moved from core)
- [ ] Delegate to core for: list, status, cancel, signal, retry, cleanup
- [ ] Documentation

**24. Node.js end-to-end tests**
- [ ] Worker E2E tests (similar to Python's test suite)
- [ ] Job execution tests
- [ ] Workflow execution tests
- [ ] Activity coordination tests
- [ ] Signal handling tests
- [ ] Failover tests

---

## **Priority 4: Testing & Quality**

**25. Integration tests for new CLI commands**
- [ ] Tests for `retry` command
- [ ] Tests for `cleanup` command
- [ ] Tests for config file loading
- [ ] Tests for config priority chain
- [ ] Tests for schema version checking

**26. Performance benchmarks for observability**
- [ ] Benchmark overhead with tracing enabled vs disabled
- [ ] Benchmark overhead with metrics enabled vs disabled
- [ ] Validate <5% overhead goal
- [ ] Document performance characteristics

**27. Migration rollback testing**
- [ ] Test down migrations
- [ ] Test version compatibility across migrations
- [ ] Document rollback procedures

---

## **Priority 5: Documentation**

**28. Observability documentation**
- [ ] How to configure OTLP endpoints
- [ ] Dashboard examples (Grafana)
- [ ] Dashboard examples (Datadog)
- [ ] SigNoz setup guide
- [ ] Common observability patterns
- [ ] Troubleshooting guide

**29. Multi-language project guide**
- [ ] How to use Python workers + Node workers
- [ ] Shared database setup
- [ ] Separate language runtimes
- [ ] Version coordination
- [ ] Example repository

**30. Production deployment guide**
- [ ] Docker/Kubernetes examples
- [ ] Rolling upgrades strategy
- [ ] HA PostgreSQL setup
- [ ] Monitoring and alerting
- [ ] Backup and recovery
- [ ] Security best practices

**31. currant.toml configuration reference**
- [ ] Complete reference documentation
- [ ] Examples for different environments
- [ ] Common patterns and recipes
- [ ] Validation and error messages

---

## **Priority 6: Future Features (Low Priority)**

These are planned but not actively worked on until Python/Node are mature.

**32. Workflow visualization dashboard**
- Not started
- Would provide visual representation of workflow execution
- See execution tree, timing, status

**33. Workflow testing utilities**
- Not started
- Helpers for testing workflows in isolation
- Mock activities, time controls

**34. Additional language adapters**
- Not started until Python/Node reach maturity
- **Go**: CGO bindings, example worker/migrate scripts
- **Rust native**: Direct library usage (no FFI)
- **Ruby**: Rutie/FFI bindings, bundle exec CLI

---

## Notes

### Architectural Decisions Reference

See `.claude/CONTEXT.md` for detailed architectural decisions including:
- CLI architecture (dual-tier: global admin + language adapters)
- Configuration management (TOML + env vars)
- Version management strategy
- Observability design principles

See `.claude/TRACING_DESIGN.md` for complete observability design including:
- Metrics to collect
- Tracing implementation
- Cross-language propagation
- OTLP export strategy

### Priority Ordering Rationale

1. **CLI & Config** (Priority 1) - Blocking architectural changes from recent decisions
2. **Observability** (Priority 2) - Critical for production usage
3. **Node.js Maturity** (Priority 3) - Second language adapter
4. **Testing** (Priority 4) - Quality assurance
5. **Documentation** (Priority 5) - User experience
6. **Future** (Priority 6) - Nice-to-have features

### How to Use This Document

- Pick items from Priority 1 first
- Mark items complete with [x] when done
- Add notes or blockers under items as needed
- Update priorities as project evolves
- Keep CONTEXT.md in sync with architectural changes
