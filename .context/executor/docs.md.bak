---

# 📄 **Part 1 — Core Design Overview**

*(All decisions here are foundational. Later parts build on this.)*

---

## ✅ **1. Purpose & Intent**

We are designing a **resumable interpreter** for a subset of JavaScript-like language.
It must:

* Execute statements one step at a time.
* Pause only at `await` points.
* Persist execution state (stack + variables) to storage.
* Restore later and continue as if it never stopped.
* Be deterministic in execution after resume (no replay).
* Be implemented in **Rust**, using **Serde** for JSON persistence.

---

## ✅ **2. What This Interpreter Is (and Isn’t)**

| ✅ Will do                                                                      | ❌ Will *not* do (at least in v1)                            |
| ------------------------------------------------------------------------------ | ----------------------------------------------------------- |
| Support `let`, `const`, blocks, if, while, assignment, return, break, continue | No `var`, no hoisting, no function-scoped vars              |
| Support `await <expr>` and `let x = await <expr>`                              | No inline-await in deep expressions (e.g. `f(await x + 1)`) |
| Allow nested statements and full stack-based execution                         | No recursive function calls yet (can be added later)        |
| Pause/resume at `await`, persist full VM state                                 | No re-evaluation of already-computed expression parts       |
| Handle only safe async operations (task scheduling only)                       | No file I/O, networking, DB handles, or resource locking    |

---

## ✅ **3. AST (Abstract Syntax Tree) Assumptions**

* **Immutable AST:** Once parsed and stored, the AST is immutable and referenced by a **version ID**. Every execution binds to exactly one version. If that row changes, it’s treated as corruption; no recovery or migration is attempted in v1.

* **No per-node IDs in v1:** We do **not** assign or persist `NodeId`s for statements/expressions. Precise resume is determined solely by the **persisted frame stack + per-statement PC enums** and the environment snapshot.

* **Snapshot contents:** Snapshots persist `frames` (each with its `FrameKind` + PC + scope base), `env` (slots + `sp`), `control`, and (only while suspended) the `await_capsule`. This is sufficient to resume exactly where execution paused.

* **Future source mapping (optional):** When needed (debugging, metrics, ANF rewrites), we can add **line/column/source map** data or lightweight node identifiers *without changing* the VM contract or snapshot semantics.

* **No hoisting or TDZ (v1):** The language subset omits `var`/function hoisting and TDZ. Declarations are validated at parse time; runtime uses slot addressing only.


---

## ✅ **4. VM Architecture — High-Level Structure**

The VM maintains:

1. **A stack of frames** – one per active statement.
   Each frame stores:

   * Which statement it represents (`Stmt`)
   * What step we’re on (`pc`)
   * Where its variables start (`scope_base_sp`)
   * (For await) resolved values and node id

2. **A flat variable stack (`env`)**, separate from the statement stack.

   * Works like `Vec<Val>` + stack pointer `sp`
   * Each block or function pushes new scope (`scope_base_sp = sp`)
   * When block/function exits or unwinds → truncate to base

3. **A control flag** (`Control`)
   Values like:

   * `None`
   * `Break`
   * `Continue`
   * `Return(Option<Val>)`
   * `Throw(Val)`

4. **Await capsule (`AwaitCapsule`)**
   Only exists when execution is suspended at an `await`:

   * NodeId of the await
   * Task policy (`Any` | `All`)
   * Task IDs (UUIDs)
   * Whether tasks were created yet

---

## ✅ **5. Stack-Based Execution — Key Insight**

Statements **don’t call each other using the system call stack**.

Instead:

* The interpreter is a loop.
* It looks at the *top frame*.
* Determines what to do next (based on statement type + PC).
* Either:

  * Pushes a new frame (nested statement),
  * Pops the frame (statement finished),
  * Updates PC and continues,
  * Or Yields (if at an `await`).

This makes execution **serializable**, because:

* Everything is explicitly in data structures (no call stack).
* Every relevant variable is in `env`, not function locals.

---

## ✅ **6. Why This Works with `await`**

Traditional interpreters pause on the call stack — which can’t be easily serialized.

Here, we:

* Restrict `await` so it only appears in **statement contexts** (`await x;` or `y = await z;`).
* When `await` happens:

  * We **don’t push another frame**.
  * We **record PC = waiting state**.
  * Snapshot frame stack + env.
  * Return control to host → store JSON.

Later:

* Reload JSON.
* Restore VM.
* Re-enter `step()` loop.
* Skip expression evaluation before the await — inject resolved value.

---

Awesome — continuing.

---

# 📄 **Part 2 — Statement Execution Model & Control Flow**

This section explains **how each statement type executes**, how **PC values (program counters)** work, and how **nested statements + resume logic** function without confusion.

---

## ✅ **7. Statement Frames & Program Counters (PC)**

Each active statement has a **Frame**, which includes:

```rust
struct Frame {
    kind: FrameKind,       // e.g. While, If, Block, Assign, etc.
    scope_base_sp: usize,  // where its variables begin in env
    node: Stmt,            // original AST node
    await_node_id: Option<NodeId>, // only used for await
    await_value: Option<Val>,      // stores resolved value if resuming
}
```

---

### ✅ **Program Counter (PC) — What It Actually Means**

Each statement type runs in **multiple phases**.
The **PC tracks which phase** we're currently in.

Example for a `while` statement:

| PC Value            | Meaning               |
| ------------------- | --------------------- |
| `WhilePc::Check`    | Evaluate condition    |
| `WhilePc::RunBody`  | Execute loop body     |
| `WhilePc::PostBody` | Handle break/continue |

These are not numbers — they’re enums:

```rust
#[repr(u8)]
enum WhilePc { Check = 0, RunBody = 1, PostBody = 2 }
```

Why enums?
✔ Self-documenting
✔ Compiler prevents invalid PC states
✔ Serialized as tiny integers — no performance loss

---

## ✅ **8. How Nested Statements Work (No Function Calls!)**

Let's say we have:

```js
while (x < 5) {
    if (y > 0) {
        y--;
    }
}
```

Execution looks like this:

1. Frame: `While { pc = Check }`
2. Condition = true → set `pc = RunBody`
3. Push child frame: `Block { pc = Enter }`
4. Block executes its statements:

   * Push `If { pc = EvalCond }`
   * If finishes → pop
   * Block finishes → pop
5. Return to `While { pc = PostBody }`
6. Handle control (break/continue/return/throw)
7. Go back to `Check`, repeat

---

### ✅ **IMPORTANT RULE: Push/Pause Pattern**

Whenever a parent needs a child statement to run:

✔ It **updates its own PC first**
✔ Then it **pushes the child frame**
✔ Then it **returns `Step::Yield`**

This means:

* When the child finishes and pops, the parent has the correct PC already set.
* When resuming (after `await`), PC is where execution should continue.

---

## ✅ **9. Centralized Control Flow Handling**

Rust enum:

```rust
enum Control {
    None,
    Break,
    Continue,
    Return(Option<Val>),
    Throw(Val),
}
```

Instead of each statement checking for these manually — we centralize:

### ✅ **Rule: Control Flow Is Handled Before Each Step**

At the start of `step(vm)`:

```rust
if vm.control != Control::None {
    if unwind(vm) {
        return Step::Continue;
    } else {
        return Step::Done;
    }
}
```

`unwind(vm)` does:

✔ Pop frames until we reach the appropriate handler:

| Control  | Stop Unwinding At           |
| -------- | --------------------------- |
| Break    | Nearest loop                |
| Continue | Nearest loop                |
| Return   | Function root / script root |
| Throw    | Nearest try block           |

✔ For every frame popped → restore environment (`env.truncate(frame.scope_base_sp)`)

✔ For `Throw` → re-enter executing at appropriate `catch` or `finally`

---

## ✅ **10. Summary of Statement PCs Per Type**

| Statement      | PC Enum                       | Phases                                       |
| -------------- | ----------------------------- | -------------------------------------------- |
| Block          | `BlockPc`                     | Enter scope → run children → exit scope      |
| If/Else        | `IfPc`                        | Evaluate condition → branch dispatch         |
| While          | `WhilePc`                     | Check → run body → post-body                 |
| Let            | no PC or simple flag          | Declare + optional initializer               |
| Assign         | `AssignPc`                    | Simple or Await-based phases                 |
| ExprStmt       | `ExprKindPc`                  | Immediate or Await-based                     |
| Try/Catch/Fin. | `TryPc`                       | EnterTry → AfterTry → Catch → Finally → Done |
| Await          | `AwaitExprPc`/`AwaitAssignPc` | CreateTasks → Waiting → Assign(or Done)      |

---

Great — continuing.

---

# 📄 **Part 3 — Await & Suspension System**

This part nails down how `await` works end-to-end: where it’s allowed, what we persist, how we resume, and how we keep it idempotent.

---

## ✅ 11. Allowed `await` Forms (v1)

To keep pause/resume trivial and side-effect safe, `await` is **restricted to statement boundaries**:

* **Expression statement:**
  `await E;`
* **Assignment statements:**
  `let x = await E;`
  `x = await E;`

> No inline `await` inside complex expressions in v1.
> (Future: add ANF desugaring to support that without changing the VM contract.)

---

## ✅ 12. Await Semantics Overview

At runtime, `await` is a **first-class yield point**. It does **not** push a child frame. The parent frame’s **PC encodes the waiting state**.

Two shapes:

* **ExprStmt(Await E)** uses `AwaitExprPc`:

  1. `CreateTasks` → 2) `Waiting` → 3) `Done`

* **Assign(name = Await E)** uses `AwaitAssignPc`:

  1. `CreateTasks` → 2) `Waiting` → 3) `Assign` → 4) `Done`

Key property: because we never push a child for `await`, there’s no ambiguity between “resume from suspension” vs “return from child.” The **PC alone** tells us which phase we’re in.

---

## ✅ 13. Suspension Capsule (Persisted Checkpoint)

When we hit an `await`, we create a **single snapshot** that records everything needed to resume:

```rust
#[derive(Serialize, Deserialize)]
pub struct AwaitCapsule {
  pub await_node_id: NodeId,     // Which await this is
  pub policy: AwaitPolicy,       // Any | All
  pub task_ids: Vec<String>,     // UUIDs for scheduled tasks
  pub created: bool,             // idempotency bit (or epoch)
  // optional: run_at timestamps, metadata for host scheduler
}
```

The **VM snapshot** includes:

```rust
#[derive(Serialize, Deserialize)]
pub struct VM {
  pub frames: Vec<Frame>,        // with per-stmt PCs
  pub env: VMEnv,                // slots + sp
  pub control: Control,          // None/Break/Return/Throw
  pub await_capsule: Option<AwaitCapsule>, // present only while suspended
}
```

> **Atomicity:** Persist this VM snapshot **in the same DB transaction** that creates the tasks (see §16).

---

## ✅ 14. Task Creation & Idempotency

**Create once, resume many** is the rule.

* On entering `CreateTasks`:

  * Evaluate any **sync** parts of `E` needed to produce tasks (but *no* side-effecting calls beyond task generation).
  * Generate `task_ids` (UUIDs).
  * Persist `{await_node_id, policy, task_ids}` inside `await_capsule`.
  * Set `created = true`.
  * **Commit snapshot and tasks atomically**.
  * Return `Yield`.

* On retry (e.g., if the host replays `CreateTasks`):

  * If a snapshot already exists with `created = true`, **do not create tasks again**. Just return `Yield`.

---

## ✅ 15. Resume Logic (Any/All)

When the host decides to resume this VM:

1. **Reload snapshot** (`VM` JSON)
2. **Poll task results** for `await_capsule.task_ids` (host-provided hook)
3. Three outcomes:

* **Policy = Any**

  * If **any** task finished successfully: inject that value; continue VM.
  * If any task **failed**: inject `Throw(err)` (engine will unwind).
  * If none finished: return `Yield` (no changes).

* **Policy = All**

  * If **all** tasks finished successfully: inject array/object of results; continue VM.
  * If any **failed**: inject `Throw(err)`.
  * Otherwise: `Yield` again.

> Injection destination:
>
> * For **assignment**: store into target slot during `Assign` phase.
> * For **expr stmt**: discard or store to a temp (implementation choice); then `Done`.

---

## ✅ 16. Atomic Snapshot + Task Creation

**Transaction boundaries:**

* Begin DB txn
* Create tasks (rows in `tasks` table)
* Write VM snapshot with `await_capsule { created = true }`
* Commit

If commit fails, **nothing** is created; on retry, we can safely run `CreateTasks` again.

If commit succeeds but network fails, we’ll **reload** a snapshot that already has `created = true`; `CreateTasks` will **not** run again.

---

## ✅ 17. Error & Cancellation Paths

* **Task failure** → resume with `Throw(err)`:

  * `vm.control = Control::Throw(err)` and proceed to centralized unwinding.
  * `try/finally` semantics apply: finally runs; catch may handle; else uncaught → stop.

* **Cancellation/Timeouts**:

  * Treat as **synthetic rejection** with a specific error code/class; the same unwinding path applies.
  * This keeps engine logic minimal and predictable.

---

## ✅ 18. Environment at Suspension

All **locals and block-scoped variables** already live in `env` (flat slots). The snapshot captures:

* `env.slots` (the values)
* `env.sp` (stack pointer)
* Any per-scope name maps (temporary until parser assigns slot indices)

On resume:

* Rehydrate `env`
* Continue from the frame’s PC
* **No need to re-evaluate pre-await expressions**, because v1 syntax forbids them (await is a statement).

---

## ✅ 19. Security & Sandbox Assumptions

* The only permitted side effect is **task creation** associated with `await`.
* No DB/file/socket handles are persisted in the VM.
* `Val` should avoid host pointers/handles; use pure data (JSON-like).
* If you later add host calls, enforce **idempotency** or confine them to **post-await** phases.

---

## ✅ 20. Minimal Host Interfaces (suggested)

These functions live outside the VM and are injected via your runtime:

```rust
/// Build tasks from an expression and return their IDs (no side effects besides task creation).
fn create_tasks_for(expr: &Expr, env: &mut VMEnv) -> Vec<String>;

/// Given an await capsule, poll for a ready value. Return:
/// - Some(Ok(val))  when ready with value
/// - Some(Err(err)) when completed with error
/// - None           when still pending
fn poll_tasks(capsule: &AwaitCapsule) -> Option<Result<Val, Val>>;
```

In the skeleton, we called these `create_tasks_for` and `try_poll_await` (the latter would use `poll_tasks` internally).

---

## ✅ 21. Example Timeline (Quick, Concrete)

**Program**

```js
let a = 1;
let r = await taskAll(fetchA(a), fetchB());
r; // expr stmt
```

**Run**

1. `Let a=1` → env[a]=1
2. `Assign r = await …`

   * PC: `CreateTasks` → build tasks T1, T2; snapshot `{await_node_id, All, [T1,T2], created=true}`; `Yield`
3. External system finishes T1 only → resume VM

   * Poll: not all done → `Yield`
4. External finishes T2 → resume VM

   * Poll: all done → inject value → `Assign` phase writes env[r]=value → `Continue`
5. `ExprStmt r;` → eval `r` → `Continue`
6. Done.

---

## ✅ 22. Why This Model Is Safe

* We **only** pause at well-defined statement boundaries (`await`), so there’s no partial expression state to reconstruct.
* The **PC** captures the micro-phase of a statement; `await` doesn’t push a child frame, removing ambiguity.
* The **env** is complete and serializable.
* All side effects (tasks) are **atomic** with the snapshot, and **idempotent** on retry.

---

Got it — you meant **Part 4 (Persistence & Resumability)**, not Section 4 of Part 1.

I'll generate **Part 4 now**, using all the recent corrections (no NodeIds, await-only-at-statement-level, etc.).

---

# 📄 **Part 4 — Persistence & Resumability**

This section defines exactly **what gets serialized**, **how suspension works**, **how resumption works**, and how we ensure **safety, idempotency, and correctness** across pause → persist → resume cycles.

---

## ✅ **4.1 What Must Be Persisted**

To fully pause and later resume execution, we persist exactly this:

| Component         | Description                                                                                                                  |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `frames`          | Stack of active statement frames. Each frame contains: statement type, program counter (PC), and its `scope_base_sp`.        |
| `env`             | Full variable environment: a flat stack of values (`slots`) + stack pointer (`sp`).                                          |
| `control`         | Any active control flow state (`None`, `Return`, `Break`, `Throw`, etc.).                                                    |
| `await_capsule`   | Only present if the VM is currently suspended on an `await`. Contains task IDs + policy (`Any` or `All`) + idempotency info. |
| **Nothing else.** | No system stack, no handles, no OS resources, no AST duplication. AST is immutable and stored separately via version ID.     |

Everything in the VM must be JSON-serializable using `serde`, so that the VM can be stored in a database row or blob.

---

## ✅ **4.2 When Persistence Occurs**

Persistence **only happens at an `await`**. Specifically:

1. A frame enters the `CreateTasks` phase (`pc = Await*Pc::CreateTasks`).
2. VM calls host-provided function to generate **task IDs**.
3. VM sets `await_capsule = Some(AwaitCapsule { policy, task_ids, created = true })`.
4. VM returns `Step::Yield`.
5. **Caller must immediately persist the VM snapshot and task rows in the same transaction.**

No other statement type causes VM persistence.

---

## ✅ **4.3 Resume Flow Overview**

When resuming:

1. Load persisted VM from storage (JSON → struct).
2. Ensure `await_capsule` exists.
3. The top frame's `pc` will be in `Waiting` phase.
4. The host checks whether the task(s) are now complete (in external task table/system).
5. Depending on the task status:

   * If **not ready**, VM returns `Step::Yield` again (no change).
   * If **ready with result**, VM injects the value, advances `pc`, clears the capsule, and continues execution.
   * If **failed**, VM converts to `control = Throw(error)` and triggers exception unwinding.

---

## ✅ **4.4 JSON Structure Example**

```json
{
  "frames": [
    {
      "kind": "Assign",
      "pc": "Waiting",          // e.g. AwaitAssignPc::Waiting
      "scope_base_sp": 0,
      "node": { ... }           // only if we store AST in frame; optional
    }
  ],
  "env": {
    "slots": [1, null, "foo"],  // JSON values for variables
    "sp": 3
  },
  "control": "None",
  "await_capsule": {
    "policy": "All",
    "task_ids": ["uuid-1", "uuid-2"],
    "created": true
  }
}
```

---

## ✅ **4.5 Atomicity & Idempotency**

When persistence happens, two things must occur **together or not at all**:

✔ VM snapshot is saved
✔ Tasks (in task table) are stored with the same IDs

This must happen **in a single transaction**.

If the DB transaction fails → neither snapshot nor tasks exist → VM remains in pre-await state → safe to retry.

If the transaction succeeds but the process crashes afterward → VM will resume later with `created = true` → snapshot prevents double task creation.

---

## ✅ **4.6 Clearing Await State After Resumption**

Once the awaited task(s) complete:

* Result is injected into frame (`await_value = Some(val)` or assigned).
* `pc` moves from `Waiting` → `Assign`/`Done` (depending on type).
* `await_capsule` is set to `None`.
* Execution continues normally until the next `await` or program termination.

---

## ✅ **4.7 What Is *Not* Persisted**

The following are **deliberately excluded** for simplicity and safety:

| Not Persisted                   | Reason                                              |
| ------------------------------- | --------------------------------------------------- |
| Call stack / Rust stack         | VM uses its own stack (`frames`) only.              |
| AST                             | Immutable + stored separately via version ID.       |
| OS handles (files, DB)          | Not allowed in sandbox. Would be invalid on resume. |
| Partially evaluated expressions | Avoided by design: `await` only at statement level. |

---

## ✅ **4.8 Resumability Guarantees**

This design guarantees:

✔ No re-execution of any code before the `await`.
✔ No lost progress — execution resumes from the correct statement and PC.
✔ No double task creation — guarded via `created = true`.
✔ No dependency on thread stack, heap pointers, or host runtime state.
✔ State can survive process crash, redeploy, or VM migration.

---

## ✅ **4.9 Optional Future Enhancements**

These are explicitly *future work*, not included in v1:

| Feature                  | Why / Use case                                 |
| ------------------------ | ---------------------------------------------- |
| Expression-level await   | Needs ANF or expression frame state.           |
| AST node IDs or line/col | For debugging, errors, stack traces.           |
| Function calls           | Needs call stack frames + return values.       |
| Hoisting / var           | Introduces TDZ and different scoping rules.    |
| External resources       | Requires handle serialization or rebind logic. |

---
