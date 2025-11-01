# Rhythm

Durable workflows using snapshots instead of replay. No determinism constraints.

Write workflows in a simple DSL (JavaScript-like async/await), implement tasks in any language (Python and Node.js supported now). When a workflow awaits, snapshot the execution state—on resume, continue from the snapshot. No event history, no determinism rules.

**Rust core with language bindings**: Single execution engine in Rust, thin FFI adapters for each language. Same workflow runs everywhere—DSL is parsed once, snapshots are universal. Fast parser (Pest-based), efficient execution.

**Also works as a task queue**: Use it for simple async tasks without workflows. Unified interface—tasks and workflows use the same execution model.

**Not** a DAG scheduler (Airflow), not a Temporal clone. This explores snapshot-based execution as an alternative to replay-based models.

**Status**: Experimental prototype. Core execution works. Missing control flow (if/else, loops), observability, production features. Expect bugs and breaking changes.

```javascript
// workflows/processOrder.flow
workflow(ctx, inputs) {
  let payment = await task("chargeCard", {
    orderId: inputs.orderId,
    amount: inputs.amount
  })

  await task("shipOrder", { orderId: inputs.orderId })
}
```

```python
# tasks.py - implement tasks in Python or JavaScript
@rhythm.task(queue="orders")
async def chargeCard(orderId: str, amount: float):
    # Normal Python - use time.now(), random(), external APIs
    # No determinism rules
    return {"success": True, "txn_id": "..."}
```

---

## Why This Exists

Temporal's replay-based model is powerful but has friction:

- Every line of workflow code must be deterministic (no `time.now()`, no random, no direct I/O)
- Versioning requires careful migration of in-flight workflows
- Mental model split: "normal code" vs "workflow code"
- Testing requires replay mocks and history simulation

Rhythm experiments with **snapshot execution state instead of replaying code**. Accept a DSL in exchange for eliminating determinism constraints.

[Read more about how this differs from Temporal →](FAQ.md#how-does-this-differ-from-temporal)

---

## What Works / What Doesn't

**Working**:
- ✅ Snapshot-based execution (await tasks, resume on crash)
- ✅ Worker failover (heartbeat-based, via Postgres)
- ✅ Python & Node.js task implementations
- ✅ Basic DSL: tasks, variables, await, fire-and-forget
- ✅ Multi-queue workers, retries, timeouts

**Missing**:
- ❌ Control flow (if/else, loops) — **biggest gap**
- ❌ Expressions (operators, property access)
- ❌ Observability (metrics, tracing, UI)
- ❌ Idempotency keys
- ❌ Rate limiting
- ❌ Production hardening (edge cases, error handling)

**Stability**: Core works. Expect rough edges, breaking changes, missing features.

---

## Quick Start

- Python (bla bla replace this)
- JS (bla bla replace this)

---

## Roadmap

**Priority 1**: Control flow (if/else, loops, expressions) — DSL unusable without this
**Priority 2**: Observability (what's running, where it's stuck)
**Priority 3**: Production features (idempotency, rate limiting, retention)

[Full roadmap](.context/TODO.md)

---

## Learn More

- **[FAQ](FAQ.md)** — Common questions, how this differs from Temporal, technical details
- **[DSL Syntax Reference](WORKFLOW_DSL_FEATURES.md)** — Complete language guide, why a DSL
- **[Technical Deep Dive](TECHNICAL_DEEP_DIVE.md)** — Snapshot model, architecture, performance
- **[Blog Post](https://...)** — Longer explanation of snapshot vs replay
- **Examples**: [Python](python/examples/) | [Node.js](node/examples/)

---

## Contributing / Feedback

Not seeking contributions yet (pre-release, rapid changes).

Feedback welcome:
- Does the snapshot model make sense?
- Is the DSL trade-off acceptable?
- What's blocking you from trying this?

[Open an issue](https://github.com/yourusername/rhythm/issues) or [discussion](https://github.com/yourusername/rhythm/discussions)
