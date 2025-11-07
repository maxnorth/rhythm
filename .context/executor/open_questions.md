# Open Questions for Executor V2

These questions will be resolved as we implement each milestone.

## Type System

**Q1: Val::Num precision**
- Should `Val::Num` be `i64` (integer only) or `f64` (floating point)?
- Decision: TBD when we implement arithmetic expressions

**Q2: Initial VM environment**
- Does `VM::new(program)` receive initial variables (like `inputs`)?
- Or does coordinator populate env before first tick()?
- Decision: TBD in Milestone 1

## Stdlib Integration (Milestone 9)

**Q3: TaskSpec return from stdlib**
- When `task.run()` is called, stdlib needs to return both Val::TaskHandle AND TaskSpec
- Should trait be: `fn call(...) -> Result<(Val, Option<TaskSpec>), Val>`?
- Or: `fn call(...) -> Result<Val, Val>` and VM inspects returned Val for TaskHandle?
- Decision: TBD in Milestone 9

**Q4: AwaitRequest structure**
```rust
pub struct AwaitRequest {
    pub wait_on: Vec<???>,  // TaskHandle objects or just Vec<String> IDs?
    pub to_create: Vec<TaskSpec>,
}
```
- Decision: TBD in Milestone 8

**Q5: TaskSpec fields**
```rust
pub struct TaskSpec {
    pub id: String,
    pub function: String,
    pub args: Vec<Val>,
    // Any other fields? (timeout, retries, queue, etc.)
}
```
- Decision: TBD in Milestone 9

## Error Handling

**Q6: Error value structure**
- Errors as `Val::Obj` with "message" and "code" keys?
- Or dedicated `Val::Error { message, code }` variant?
- Decision: TBD in Milestone 7 (try/catch)

## Expression Evaluation

**Q7: Member expression this-binding**
- How does `eval_expr(Member { obj, prop })` communicate the base value for method calls?
- Return tuple `(Val, Option<&Val>)` where second is `this`?
- Or separate evaluation path for callee vs other expressions?
- Decision: TBD in Milestone 9 (stdlib integration)

---

**Note**: This file will be updated as we resolve questions during implementation.
