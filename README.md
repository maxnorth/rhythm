# Rhythm - Intuitive, Effortless Durable Execution

Rhythm is a Durable Execution framework with a novel architecture that makes writing Workflows-as-Code intuitive and effortless. It’s easy to setup and use, requiring only Postgres and a package install. It supports multiple programming languages, each using a shared core engine written in rust.

> [!WARNING]
> The project is still in early development. It's usable, but missing many features, and is not battle tested for production. It's recommended to only use it for experimental evaluation or hobby projects at this time.


## Example
```js
// workflows/onboard_user.flow

const user = await Task.run("get-user", { userId: Inputs.userId })
const account = await Task.run("create-billing-account", { user })
await Task.run("send-welcome-email", { user, account })

return {
  userId: user.id,
  accountId: account.id,
  status: "onboarded"
}
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
