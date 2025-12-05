# Rhythm - Intuitive, Effortless Durable Execution

Rhythm is a Durable Execution framework that aims to make writing Workflows-as-Code intuitive and effortless. It’s easy to setup and use, requiring only Postgres as the single hosted dependency. It supports multiple programming languages, each using Rhythm's shared core engine written in rust.

> [!WARNING]
> The project is still in early development. It's usable but missing many features, and is not battle tested for production. It's exclusively recommended for experimental evaluation or hobby projects at this time.

## Key Features
- Immutable, self-versioning workflows written in a JS-based DSL
- Pauses and resumes when you `await` - no replay or determinism traps
- Define and run tasks in your application's language
- 2-in-1 framework for workflows and background tasks, with unified scheduling and monitoring
- SDK's currently available for Python and JavaScript, with more planned

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

```py
# app/tasks.py

from rhythm import RhythmApp, task

app = RhythmApp()

@task("get-user")
def get_user(payload: dict) -> dict:
    user_id = payload["userId"]
    user = db.fetch_user(user_id)
    return {"id": user.id, "email": user.email}


@task("create-billing-account")
def create_billing_account(payload: dict) -> dict:
    user = payload["user"]
    account = billing.create_account(email=user["email"])
    return {"id": account.id}


@task("send-welcome-email")
def send_welcome_email(payload: dict) -> dict:
    user = payload["user"]
    account = payload["account"]
    mailer.send(
        to=user["email"],
        template="welcome",
        context={"accountId": account["id"]},
    )
    return {"sent": True}
```

```py
# app/main.py

from rhythm import RhythmClient

client = RhythmClient()

result = await client.start_workflow(
    "onboard_user",
    {"userId": 42}
)
```

## Architecture at a glance

**Workflow DSL:** Workflows are written in a custom DSL stored in .flow files, based on a simplified subset of JavaScript. Rhythm 

**Immutable, self-versioning workflows:** During app init, Rhythm stores the source of workflows in Postgres, versioned by hash. In-flight workflows resume using the same version they started with. New workflow runs use latest by default.

**Single dependency:** All state, scheduling, and results are stored in Postgres. All execution happens inside your service. Compare to a simple Postgres-backed task queue.

Tasks: Regular functions in your application, written in your language, invoked from workflows via adapters (e.g. Task.run("send-email", payload)).

**No replay:** Workflow code is not replayed from history; it’s a sequential interpreter that can pause/snapshot/resume, not a deterministic replay engine.

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
