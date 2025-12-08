# Rhythm - Intuitive Durable Execution

Rhythm is a Durable Execution framework that aims to make writing Workflows-as-Code intuitive and effortless. It’s easy to setup and use, and only requires Postgres as the single hosted dependency. It supports multiple programming languages, each using Rhythm's embedded core engine written in rust.

Rhythm may appeal to you if:
- You have experienced how replay-based durable execution complicates maintainability and reliability
- You want a free, low-maintenance platform designed for developers, without commercial constraints
- You don't want to monitor and maintain separate platforms for durable workflows *and* queued tasks

> [!WARNING]
> This project is in early access. It's usable but missing many features, and is not battle tested for production. Backwards compatibility is not guaranteed. It's exclusively recommended for evaluation or hobby projects at this time. [Learn more.](./docs/release_status.md)

## Quickstart
See below for setup instructions and examples in your chosen app language.
- [Python](./python/README.md)

## How it Works
- You write workflows in `.flow` files, which use a JS-like, sandboxed scripting language to run tasks asynchronously and wait on external signals or timers of any duration.
- Rhythm's rust-based interpreter runs your workflows. When you `await`, it pauses and saves state, and when the result is resolved, the workflow restores state and resumes exactly where it left off, like normal code.
- You define tasks in your application's language. These run when invoked by a workflow, or they can be run directly as a standalone queued task.
- Workflow files are persisted and automatically versioned by their content hash. In-progress workflows are guaranteed to resume with the same version they started with, making file changes safe and effortless.
- Because workflows do not use event replay to restore state like other durable execution platforms, they do not have the same event limits or determinism requirements.

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

// capture outputs, to fetch via API, or returned to parent workflow
return {
  example: 'whatever'
}
```

## Learn More

- **[FAQ](FAQ.md)**
- **[Workflow Syntax and API Reference](WORKFLOW_DSL_FEATURES.md)**
- **[Supported Languages](docs/languages.md)**
- **[Technical Architecture](TECHNICAL_DEEP_DIVE.md)**

---

## Contributing / Feedback

Not seeking contributions yet (pre-release, rapid changes).

Feedback welcome:
- Does the snapshot model make sense?
- Is the DSL trade-off acceptable?
- What's blocking you from trying this?

[Open an issue](https://github.com/yourusername/rhythm/issues) or [discussion](https://github.com/yourusername/rhythm/discussions)
