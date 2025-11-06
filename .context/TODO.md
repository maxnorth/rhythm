# Rhythm - TODO List

> **Last Updated**: 2025-10-20
> **Status**: DSL workflows implemented, Python @workflow removed, Node.js in progress

This document tracks missing functionality and planned features for Rhythm. Items are prioritized based on recent architectural decisions and project maturity needs.

**Recent Updates**:
- 2025-10-20: **Completed DSL workflow pivot** - Removed Python @workflow decorator, Python/Node now DSL-only
- 2025-10-20: Added DSL Workflow Completion as Priority 1 (control flow, expressions, error handling)
- 2025-10-12: Added Priority 1 items for idempotency and rate limiting (see IDEMPOTENCY_DESIGN.md and RESEARCH_FINDINGS.md)
- 2025-10-11: Initial CLI architecture and observability priorities

**Quick Summary of Priority 1 Work**:
- Implement task_id (idempotency key) with optional UUID generation
- Add result storage (automatic based on return value)
- Implement "Allow Duplicate Failed Only" policy (like Temporal)
- Add retention configuration per queue (default: tasks=7d, workflows=30d)
- Implement queue-level rate limiting (token bucket, zero-cost when disabled)
- Unify Activities and tasks (drop @activity decorator)
- Enable activity queue routing for rate limiting

**Total Priority 1 Items**: 12 major items with ~60+ subtasks

---

## **Priority 1: DSL Workflow Completion (Critical - NEW)**

The basic DSL implementation is working but needs core features to be production-ready.

### Control Flow

**1. Implement if/else conditionals**
- [ ] Parser: Add if/else statement parsing
- [ ] AST: Define IfStatement struct with condition, then_branch, else_branch
- [ ] Executor: Implement conditional evaluation
- [ ] Support comparison operators: `==`, `!=`, `<`, `>`, `<=`, `>=`
- [ ] Support logical operators: `&&`, `||`, `!`
- [ ] Tests for conditionals

**2. Implement loops**
- [ ] Parser: Add while loop parsing
- [ ] Parser: Add for loop parsing (range-based)
- [ ] AST: Define WhileStatement and ForStatement
- [ ] Executor: Implement loop execution with state preservation
- [ ] Executor: Support break/continue (optional)
- [ ] Prevent infinite loops (max iterations limit)
- [ ] Tests for loops

**3. Implement expressions and operators**
- [ ] Parser: Add arithmetic operators: `+`, `-`, `*`, `/`, `%`
- [ ] Parser: Add string concatenation
- [ ] Parser: Add variable references
- [ ] Executor: Evaluate expressions
- [ ] Support for accessing nested object properties: `result.status`
- [ ] Support for array indexing: `items[0]`
- [ ] Tests for expressions

### Sleep Implementation

**4. Implement sleep scheduling**
- [ ] Design: Determine sleep storage mechanism (scheduled_executions table or executions.scheduled_at?)
- [ ] Executor: Store sleep wakeup time when encountering sleep()
- [ ] Worker: Check for ready-to-wake workflows
- [ ] Database: Add index for efficient sleep queries
- [ ] Tests for sleep scheduling

### Error Handling

**5. Implement task error handling**
- [ ] Design: Task return values include success/error status
- [ ] Executor: Check task result status
- [ ] Allow workflows to check: `if (result.success) { ... }`
- [ ] Tests for error handling patterns

### Usability

**6. Better error messages**
- [ ] Parser: Clear error messages with line numbers
- [ ] Executor: Helpful runtime errors (e.g., "Task 'foo' not found")
- [ ] Validation: Detect undefined variables
- [ ] Validation: Type checking for task inputs

**7. DSL workflow testing utilities**
- [ ] Test helper: Mock tasks for workflow testing
- [ ] Test helper: Fast-forward sleep() calls
- [ ] Test helper: Inject task results
- [ ] Documentation and examples

---

## **Priority 2: Idempotency & Rate Limiting (Critical)**

These are foundational features needed for production readiness. Full design in `.claude/IDEMPOTENCY_DESIGN.md`.

### Idempotency Implementation

**1. Database schema for idempotency**
- [ ] Add `task_id VARCHAR(255)` column to executions table
- [ ] Add `result JSONB` column to executions table
- [ ] Create unique index: `idx_task_id_active` (with status filter for failed/cancelled/timed_out)
- [ ] Create index: `idx_executions_retention` for cleanup queries
- [ ] Migration script for schema changes
- [ ] Update Rust types in `core/src/types.rs`

**2. Task ID generation and deduplication**
- [ ] Generate UUID for `task_id` if not provided by user
- [ ] Implement duplicate detection on enqueue (PostgreSQL `ON CONFLICT`)
- [ ] Return existing execution for duplicates (pending/running/completed)
- [ ] Allow retry for failed/cancelled/timed_out (new execution with same task_id)
- [ ] Store result JSONB on task completion (if function returns value)
- [ ] Return cached result for duplicate requests

**3. ID reuse policy (hardcoded for v1)**
- [ ] Implement "Allow Duplicate Failed Only" behavior
- [ ] Block duplicates for: pending, running, completed
- [ ] Allow duplicates for: failed, cancelled, timed_out
- [ ] Clear error messages for each case
- [ ] Tests for all status combinations

**4. Retention configuration**
- [ ] Add `retention` field to queue config (TOML + programmatic)
- [ ] Default retention: tasks=7d, workflows=30d
- [ ] Parse duration strings ("7d", "30d", "365d", "0" for immediate)
- [ ] Background cleanup task (runs hourly)
- [ ] Per-queue retention cleanup logic
- [ ] Tests for retention cleanup

**5. Task API updates**
- [ ] Add `task_id` parameter to `enqueue()` (optional)
- [ ] Add `task_id` parameter to `execute_activity()` (optional)
- [ ] Return execution handle with result retrieval
- [ ] Update Python adapter API
- [ ] Update Node adapter API (when implemented)
- [ ] Documentation and examples

### Rate Limiting Implementation

**6. Queue configuration for rate limiting**
- [ ] Add `rate_limit` field to queue config (e.g., "100/sec", "1000/min")
- [ ] Parse rate limit strings into tokens + duration
- [ ] Store queue config in database or load from rhythm.toml
- [ ] Zero-cost check: if no rate_limit, skip all rate limiting logic
- [ ] Tests for config parsing

**7. Token bucket rate limiter**
- [ ] Create `rate_limits` table (queue, tokens, last_refill)
- [ ] Implement token bucket algorithm in Rust
- [ ] Check rate limit before claiming task (if queue has rate_limit)
- [ ] Update tokens atomically on successful claim
- [ ] Refill tokens based on elapsed time
- [ ] Tests for rate limiting behavior

**8. Rate limiting optimizations**
- [ ] Fast path: direct claim if queue has no rate_limit
- [ ] Slow path: check rate_limit table first, then claim
- [ ] Partial index for fast claims: `WHERE status = 'pending'`
- [ ] Target performance: 500-1000 tasks/sec per worker (with rate limiting)
- [ ] Performance benchmarks

**9. Queue auto-creation**
- [ ] Allow tasks to reference queue by string (no pre-definition needed)
- [ ] Queues auto-create on first use
- [ ] Apply config from rhythm.toml if exists
- [ ] Default config if not in rhythm.toml
- [ ] Tests for auto-creation

### Tasks = Tasks Unification

**10. Unify Activities and tasks**
- [ ] Remove `@rhythm.task` decorator
- [ ] Tasks can be called standalone or from workflows
- [ ] Add `parent_workflow_id` column to executions (nullable)
- [ ] Tasks called from workflows populate `parent_workflow_id`
- [ ] `execution_type` field: 'task' or 'workflow_task' (or keep as 'task'/'task' internally)
- [ ] Tests for both usage patterns

**11. Queue routing for Activities**
- [ ] Tasks declare queue via decorator: `@rhythm.task(queue="stripe-api")`
- [ ] Tasks use task's queue (not workflow's queue)
- [ ] Rate limiting applies to task tasks
- [ ] Tests for cross-queue task calls

---

## **Priority 3: CLI Architecture & Configuration (Critical)**

These items reflect recent architectural decisions (2025-10-11) that need implementation.

### CLI Restructuring

**1. Global admin CLI binary** (`rhythm-cli`)
- [ ] Create separate cargo package for optional global installation
- [ ] Include commands: `list`, `status`, `cancel`, `signal`, `retry`, `cleanup`
- [ ] Exclude: `migrate`, `worker`, `bench` (language-specific)
- [ ] Update CONTEXT.md if architecture changes

**2. Remove `migrate` from core CLI**
- [ ] Remove `migrate` command from `core/src/cli.rs` (currently lines 150-154)
- [ ] Keep in Python adapter (`python/rhythm/__main__.py`)
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

**6. `rhythm.toml` configuration file support**
- [ ] Add `toml` crate to core dependencies
- [ ] Implement config loading in Rust core
- [ ] Search path: `rhythm.toml` → `~/.config/rhythm/config.toml`
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
- [ ] Add `rhythm_metadata` table with key-value storage
- [ ] Store schema version on each migration
- [ ] Add `get_schema_version()` function in core
- [ ] Version checking on CLI commands (warn on mismatch)
- [ ] Migration to create metadata table
- [ ] Document version compatibility strategy

---

## **Priority 4: Observability System (Metrics & Tracing)**

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
- [ ] `rhythm.executions.claimed` (labels: queue, worker_id, execution_type)
- [ ] `rhythm.executions.completed` (labels: queue, status, execution_type)
- [ ] `rhythm.executions.created` (labels: queue, execution_type, has_parent)
- [ ] `rhythm.workflow.replays` (labels: workflow_name)

**12. Implement core histograms**
- [ ] `rhythm.execution.duration` (labels: execution_type, queue, status)
- [ ] `rhythm.claim_loop.duration` (labels: worker_id, queue)
- [ ] `rhythm.db.query.duration` (labels: operation)

**13. Implement core gauges**
- [ ] `rhythm.workers.active` (from worker_heartbeats table)
- [ ] `rhythm.executions.waiting` (labels: queue, execution_type)
- [ ] `rhythm.executions.running` (labels: queue, execution_type, worker_id)

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
  - `rhythm.function.execution.duration`
  - `rhythm.serialization.duration`
  - `rhythm.ffi.calls`
- [ ] Configuration via Worker constructor
- [ ] Documentation and examples

**17. Node.js OpenTelemetry integration**
- [ ] Add OpenTelemetry SDK dependencies
- [ ] Similar implementation to Python
- [ ] OTLP export configuration
- [ ] Documentation and examples

**18. Observability configuration API**
- [ ] Support env vars:
  - `RHYTHM_TRACING_ENABLED`
  - `RHYTHM_TRACING_ENDPOINT`
  - `RHYTHM_TRACES_SAMPLE_RATE`
  - `RHYTHM_METRICS_ENABLED`
  - `RHYTHM_METRICS_ENDPOINT`
  - `RHYTHM_SERVICE_NAME`
- [ ] Support programmatic config in Worker constructor
- [ ] Support `rhythm.toml` observability section
- [ ] Validate config and provide helpful errors

### Advanced Observability (Optional)

**19. Custom instrumentation API**
- [ ] Expose span creation API to user code
- [ ] Expose metrics API to user code
- [ ] Python: `rhythm.trace.span()`, `rhythm.metrics.increment()`
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

## **Priority 5: Node.js Adapter Maturity**

Bring Node.js adapter to parity with Python.

**23. Node.js CLI implementation**
- [ ] Create CLI entry point (`npx rhythm <command>`)
- [ ] Implement `worker` command handler
- [ ] Implement `bench` command handler (when bench moved from core)
- [ ] Implement `migrate` command handler (when migrate moved from core)
- [ ] Delegate to core for: list, status, cancel, signal, retry, cleanup
- [ ] Documentation

**24. Node.js end-to-end tests**
- [ ] Worker E2E tests (similar to Python's test suite)
- [ ] Task execution tests
- [ ] Workflow execution tests
- [ ] Task coordination tests
- [ ] Signal handling tests
- [ ] Failover tests

---

## **Priority 6: Testing & Quality**

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

## **Priority 7: Documentation**

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

**31. rhythm.toml configuration reference**
- [ ] Complete reference documentation
- [ ] Examples for different environments
- [ ] Common patterns and recipes
- [ ] Validation and error messages

---

## **Priority 8: Future Features (Low Priority)**

These are planned but not actively worked on until Python/Node are mature.

**32. Workflow visualization dashboard**
- Not started
- Would provide visual representation of workflow execution
- See execution tree, timing, status

**33. Workflow testing utilities**
- Not started
- Helpers for testing workflows in isolation
- Mock Tasks, time controls

**34. Additional language adapters**
- Not started until Python/Node reach maturity
- **Go**: CGO bindings, example worker/migrate scripts
- **Rust native**: Direct library usage (no FFI)
- **Ruby**: Rutie/FFI bindings, bundle exec CLI

---

## Notes

### Future Refactoring Items

**Database schema cleanup** (Low priority - breaking change):
- [ ] Rename `executions.kwargs` to `executions.inputs` for consistency with DSL workflows
- [ ] Drop `executions.args` column (unused, deprecated)
- [x] Drop `workflow_execution_context.statement_index` column (replaced by ast_path) - Migration 6

### Architectural Decisions Reference

See `.claude/CONTEXT.md` for detailed architectural decisions including:
- CLI architecture (dual-tier: global admin + language adapters)
- Configuration management (TOML + env vars)
- Version management strategy
- Observability design principles

See `.claude/IDEMPOTENCY_DESIGN.md` for complete idempotency design including:
- Task ID (idempotency key) behavior
- Retention and deduplication windows
- ID reuse policy ("Allow Duplicate Failed Only")
- Result storage and caching
- Rate limiting approach (queue-level, token bucket)
- Tasks = tasks unification
- Performance targets and optimizations

See `.claude/TRACING_DESIGN.md` for complete observability design including:
- Metrics to collect
- Tracing implementation
- Cross-language propagation
- OTLP export strategy

See `.claude/RESEARCH_FINDINGS.md` for competitive analysis including:
- How Temporal, DBOS, Celery, Sidekiq, BullMQ handle idempotency
- Rate limiting implementations across platforms
- 35 prioritized features from mature workflow systems
- Feature comparison matrix

### Priority Ordering Rationale

1. **DSL Workflow Completion** (Priority 1) - Core feature, architectural foundation, blocking adoption
2. **Idempotency & Rate Limiting** (Priority 2) - Foundation for production reliability, prevents duplicate operations
3. **CLI & Config** (Priority 3) - Blocking architectural changes from recent decisions
4. **Observability** (Priority 4) - Critical for production usage
5. **Node.js Maturity** (Priority 5) - Second language adapter
6. **Testing** (Priority 6) - Quality assurance
7. **Documentation** (Priority 7) - User experience
8. **Future** (Priority 8) - Nice-to-have features

### How to Use This Document

- Pick items from Priority 1 first
- Mark items complete with [x] when done
- Add notes or blockers under items as needed
- Update priorities as project evolves
- Keep CONTEXT.md in sync with architectural changes
