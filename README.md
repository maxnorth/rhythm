# Rhythm - Intuitive Durable Execution

Rhythm is an experimental Durable Execution framework that can resume without replaying event history. It aims to make writing durable workflows intuitive and effortless. It has minimal hosting requirements, and only requires Postgres. It supports multiple programming languages, each using Rhythm's embedded core engine written in rust.

Rhythm may appeal to you if:
- You have experienced how replay-based durable execution complicates development and causes errors
- You want a developer-friendly platform with minimal hosting requirements
- You would like a unified platform that supports both durable workflows *and* simple queued tasks

> [!WARNING]
> This project is in early access. It's usable but missing many features, and is not battle tested for production. Backwards compatibility is not guaranteed. It's exclusively recommended for evaluation or hobby projects at this time. [Learn more.](./docs/release_status.md)

## Quickstart
See below for setup instructions and examples in your chosen app language.
- [Python](./python/README.md)

## How it Works
- Rhythm uses an embedded scripting language for workflows. The runtime has been specially designed to allow execution to pause when you `await`, persist runtime state to the DB, and later resume exactly where it left off, avoiding the need for event replay.
- Workflows are written in `.flow` files and use a custom syntax based on a simplified subset of JavaScript. Each workflow runs in a sandboxed, intentially limited context to keep logic focused on task orchestration.
- Tasks are written in your application's language. These run when invoked by a workflow, or they can be run directly as a standalone queued task. Both workflows and tasks support the same set of features for scheduling, prioritization, etc.
- Workflow scripts are self-versioning and immutable. They are persisted to the database at startup and are versioned by their content hash. In-progress workflows are guaranteed to resume with the same version they started with, allowing you to safely modify and re-deploy workflows even while they are executing.
- Because workflows don't use event replay to restore state, they don't have the same event limits or determinism requirements that other durable execution platforms do.

## Workflow Example
```js
// workflows/onboard_user.flow

// run task and wait for result
let report = await Task.run("submit-report", { reportId: Inputs.reportId })

// fire and forget tasks (no await)
Task.run("something-else", { report })

// use loops
for (let item of report.approvers) {
    await Task.run("another-thing", { item })
}

// use try/catch for reliable error handling
try {
    await Signal.when("manager-approval", { timeout: "24h" })
} catch (err) {
  return await Task.run("fail-submission", {
    draftId: draft.id,
    reason: err.type
  })
}

// capture outputs to read later, or returned to a parent workflow
return {
  example: 'whatever'
}
```

## Learn More

- **[Workflow Syntax and API Reference](docs/workflow-api.md)**
- **[Supported Languages](docs/languages.md)**
- **[Ready vs. Planned Functionality](docs/release_status.md)**

---

## Contributing / Feedback

Not seeking contributions yet (pre-release, rapid changes).

Feedback welcome:
- Does the runtime model make sense?
- Is the custom scripting language trade-off worth it?
- What's blocking you from trying this?

[Open an issue](https://github.com/maxnorth/rhythm/issues) or [discussion](https://github.com/maxnorth/rhythm/discussions)
