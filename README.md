# Rhythm - Intuitive, Effortless Durable Execution

Rhythm is a Durable Execution framework that aims to make writing Workflows-as-Code intuitive and effortless. It’s easy to setup and use, requiring only Postgres as the single hosted dependency. It supports multiple programming languages, each using Rhythm's shared core engine written in rust.

Rhythm may appeal to you if:
- You have seen how replay-based durable execution can be a risk to maintainability and reliability
- You want a free and manageable platform whose design isn't constrained by the needs of VC-backed startups
- You don't want to monitor and maintain separate platforms for durable workflows *and* queued tasks

> [!WARNING]
> The project is still in early development. It's usable but missing many features, and is not battle tested for production. It's exclusively recommended for experimental evaluation or hobby projects at this time.

## How it Works
- You write workflows in `.flow` files, which use a JS-like scripting language to run tasks and wait on external signals or timers of any duration.
- Rhythm's rust-based interpreter runs your workflows. When you `await`, it pauses and saves state, and when the result is resolved, the workflow restores state and resumes where it left off.
- You define tasks in your application's language. These run when invoked by a workflow, or they can be run directly as a standalone queued task.
- Workflow files are persisted and automatically versioned by their content hash. In-progress workflows are guaranteed to resume with the same version they started with, making file changes safe and effortless.
- Because workflows do not use event replay to restore state like other durable execution platforms, they do not have the same event limits or determinism requirements.

## Example
```js
// workflows/onboard_user.flow

await Task.run("submit-report", { reportId: Inputs.reportId })

try {
    await Signal.when("manager-approval", { timeout: "24h" })
} catch (err) {
  return await Task.run("fail-submission", {
    draftId: draft.id,
    reason: err.type
  })
}

return await Task.run("publish-submission", {
  draftId: draft.id,
  approvedBy: approval.managerId
})
```

## Setup
See below for setup instructions and examples in your chosen app language.
- [Python](./python/README.md)

## Planned Features
- Timed delays in workflows (minutes, hours, days, weeks, etc.)
- Waiting on signals, for human-in-the-loop workflows
- CRON scheduled workflows
- `Task.any(...)`, `Task.all(...)`, and `Task.race(...)` composites
- Observability, including OTEL tracing, metrics, and logs
- IDE language server and breakpoint debugger for `.flow` files
- Type-safety for `.flow` files
- Additional SDK's, including Golang, Java, and Ruby

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
