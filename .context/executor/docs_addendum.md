This supplements the original design document. It contains **all new decisions**, **clarifications**, and **design changes** introduced since then.

---

## ### ✅ 1. Executor Purity & Separation of Concerns

**Original spec:** Executor had `Host` trait with `create_tasks_for()` and `poll_tasks()`.
**Updated decision:**

* The executor **should not know about task creation, databases, or polling**.
* Its only job is to **run statements until it reaches an `await` or finishes**.

**Executor now follows this contract:**

```rust
fn tick(vm: &mut VM, input: ExecInput, stdlib: &mut impl SyncStdlib) -> ExecOutput
```

Where:

| Type                                      | Purpose                                                     |
| ----------------------------------------- | ----------------------------------------------------------- |
| `ExecInput::None`                         | First run or simple continuation.                           |
| `ExecInput::AwaitReady(Result<Val, Val>)` | Resume after a task is complete.                            |
| `ExecOutput::Continue`                    | Still executing, call `tick()` again.                       |
| `ExecOutput::Await(AwaitRequest)`         | Executor hit an `await`, execution is suspended.            |
| `ExecOutput::Done(Exit)`                  | Program finished (`return`, normal end, or uncaught error). |

---

## ### ✅ 2. How Await & Tasks Work Now

**Old model:** Executor created tasks itself and stored task IDs in its state.
**New model:**

* **Executor never creates tasks**.
* Task creation is triggered by the coordinator **only when an `Await` is returned**.
* The executor instead **pushes TaskSpecs into an in-memory “outbox”**, but does not create them.

**Execution flow now:**

| When...                | Executor Does                                                                             | Coordinator Does                                                 |
| ---------------------- | ----------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| `task.run()` is called | Returns a `TaskHandle` value and appends a `TaskSpec` to `vm.outbox`.                     | Nothing yet.                                                     |
| An `await` is reached  | Stops, returns `Await { wait_on: handles, to_create: vm.outbox }`, and clears the outbox. | Creates all tasks in `.to_create`, persists VM + tasks.          |
| Tasks complete         | Coordinator loads VM, calls `tick(vm, ExecInput::AwaitReady(_))`.                         | Executor injects resolved value and continues.                   |
| Program ends           | Returns `Done`.                                                                           | Coordinator creates any “fire and forget” tasks still in outbox. |

---

## ### ✅ 3. Fire-and-Forget Task Creation Is Supported

**Why this was added:**
We want to allow:

```js
task.run("A");   // fire & forget (no await)
task.run("B");  
let x = await task.run("C"); // only this one is awaited
```

**This is now possible because:**

* All `task.*` calls synchronously produce a `TaskHandle` **and** append a `TaskSpec` to the VM’s `outbox`.
* All TaskSpecs are created later in one batch — at the next `await`, or at program end.

---

## ### ✅ 4. Function Call Resolution Architecture

**Original spec:** functions were resolved using composite identifiers like `"task.run"` split by dots.
**Revised spec:**

* We now use **real objects with methods**, not composite names.
* **Function call resolution now works like JavaScript:**

  * `task.run()` is resolved as a **Member Expression → Call Expression**.
  * `eval_expr(Member(obj, "run"))` produces `(base_value=obj, method_name="run")`.
* To support this:

  ```rust
  Expr::Member { obj: Box<Expr>, prop: String }
  Expr::Call   { callee: Box<Expr>, args: Vec<Expr> }
  ```
* During evaluation:

  * If callee is `Member`, evaluate `obj` first → this becomes the `this` value.
  * Then dispatch to:

    ```rust
    stdlib.call(this: Option<&Val>, method_name: &str, args: &[Val])
    ```

---

## ### ✅ 5. Stdlib API (Sync-Only)

Executor calls stdlib only for **pure synchronous functions** (no DB, no async).

```rust
pub trait SyncStdlib {
    fn call(
        &mut self,
        this: Option<&Val>,
        name: &str,
        args: &[Val],
    ) -> Result<Val, Val>;
}
```

* If method is a `task.*` method → it returns `(Val::TaskHandle, TaskSpec)` and executor pushes TaskSpec to outbox.
* Any error → `Err(Val)` becomes `Throw(Val)`.

