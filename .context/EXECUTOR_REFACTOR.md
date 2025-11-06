# Workflow Executor Refactoring Plan

## Vision

Transform the workflow executor from a statement-by-statement execution engine with scattered task handling into a clean expression-based evaluation system that properly separates:

1. **Expression evaluation** - Pure computation that returns values or suspends
2. **Statement execution** - Actions that use expression results to advance program state

### Core Principles

- **Expression evaluation is pure** - `evaluate_expression()` doesn't modify `ast_path` or create side effects
- **Only await expressions suspend** - All other expressions evaluate to values immediately
- **Minimal state persistence** - Only write to DB when suspending or completing, not for intermediate progress
- **Idempotent resumption** - Re-executing at same `ast_path` checks existing suspended task rather than creating new one
- **No backwards compatibility** - Clean slate, forward-only design

## Key Architectural Changes

### 1. AST Path Tracking

**Current:** `statement_path` (misleading name - actually tracks any AST node)

**New:** `ast_path`

- Better reflects that we're tracking position in the Abstract Syntax Tree
- Used for both statements AND expressions
- Format: dot-separated paths like `"1.then_statements.0"` or `"2.body_statements.1.expression"`

### 2. Expression Evaluation System

**New concept:** `ExpressionResult` enum

```rust
enum ExpressionResult {
    Value(JsonValue),           // Expression completed, here's the value
    Suspended(String),          // Expression suspended, here's the task_id
}
```

**New function:** `evaluate_expression(expr: &JsonValue, locals: &JsonValue) -> ExpressionResult`

This function:
- Takes an expression AST node and current locals
- Returns either a computed value OR signals suspension
- Does NOT modify `ast_path`
- Does NOT create database records (caller handles that)
- Is pure and can be called multiple times safely

### 3. Suspended Task Storage

**Problem:** Can't re-evaluate `Task.run("foo", ...)` or it creates a new task

**Solution:** Store the suspended task structure in `locals.__suspended_task`

When an `await` expression suspends, we store the Task structure:

```json
{
  "__suspended_task": {
    "type": "run",
    "task_id": "uuid-123"
  }
}
```

For composed tasks:

```json
{
  "__suspended_task": {
    "type": "all",
    "tasks": [
      {"type": "run", "task_id": "uuid-1"},
      {
        "type": "any",
        "tasks": [
          {"type": "run", "task_id": "uuid-2"},
          {"type": "run", "task_id": "uuid-3"}
        ]
      }
    ]
  }
}
```

On resumption:
1. Check if `__suspended_task` exists in locals
2. Query task statuses from DB
3. If all complete, compute result and return `Value`
4. If any still pending, return `Suspended`

### 4. Task Types

All tasks are recursively composable:

- `run` - Single task execution: `Task.run("task_name", inputs)`
- `delay` - Time delay: `Task.delay(milliseconds)`
- `all` - Wait for all: `Task.all([task1, task2, ...])`
- `any` - Wait for first: `Task.any([task1, task2, ...])`
- `race` - Wait for first success or all failures: `Task.race([task1, task2, ...])`

These can nest arbitrarily:
```javascript
await Task.all([
  Task.run("foo", {}),
  Task.any([
    Task.run("bar", {}),
    Task.all([
      Task.run("baz", {}),
      Task.delay(1000)
    ])
  ])
])
```

### 5. Error Handling

**Current behavior:** Failed tasks terminate the workflow

**Future behavior (after try-catch):** Failed tasks return error values that can be caught

For now: Keep failing the workflow on task failure, but structure code to make try-catch easy to add later.

### 6. Database Persistence Strategy

**Current:** Writes to DB after every statement advancement

**New:** Only write to DB when:
- **Suspending** - Save `ast_path`, `locals`, and suspended state for resumption
- **Completing** - Save final result and mark execution complete

No tracking of intermediate progress. The executor should run through statements in-memory until it hits an await that suspends or completes the workflow.

## Refactoring Phases

### Phase 1: Rename `statement_path` to `ast_path` ✓

- [x] Create database migration to rename column
- [x] Create database migration to drop `statement_index`
- [x] Update `executor.rs` - all variable names
- [x] Update `workflows.rs` - SQL queries
- [x] Rename `get_statement_at_path()` to `get_node_at_path()`
- [x] Update comments to reference "AST path" instead of "statement path"

### Phase 2: Create Expression Evaluation Infrastructure

- [ ] Define `ExpressionResult` enum
- [ ] Create `evaluate_expression()` function signature
- [ ] Implement non-suspending expression types:
  - [ ] Literals (string, number, boolean, null)
  - [ ] Variable references
  - [ ] Object literals `{...}`
  - [ ] Array literals `[...]`
  - [ ] Property access `obj.prop`
  - [ ] Binary operations (`+`, `-`, `*`, `/`, `>`, `<`, `==`, etc.)
  - [ ] Logical operations (`&&`, `||`, `!`)
  - [ ] Function calls (stdlib functions)
  - [ ] Task expressions (without await) - just build Task structure

### Phase 3: Implement Await Expression Evaluation

- [ ] Add `__suspended_task` storage to locals
- [ ] Implement await expression handling in `evaluate_expression()`:
  - [ ] Check for existing `__suspended_task` in locals
  - [ ] If exists, query task status from DB
  - [ ] If doesn't exist and expression is `Task.run()`, create task
  - [ ] Return `Suspended(task_id)` or `Value(result)`
- [ ] Handle composed tasks:
  - [ ] `Task.all()` - check all task statuses
  - [ ] `Task.any()` - check if any completed
  - [ ] `Task.race()` - check for first success or all failures
- [ ] Failed tasks return error values (for now, still fail workflow)

### Phase 4: Update Statement Handlers

- [ ] Refactor each statement type to use `evaluate_expression()`:
  - [ ] Assignment statements
  - [ ] If/else statements (condition evaluation)
  - [ ] For loop statements (iterable evaluation)
  - [ ] Return statements
  - [ ] Expression statements
- [ ] Handle `ExpressionResult`:
  - [ ] `Value` - continue with the value
  - [ ] `Suspended` - persist state and return `StepResult::Suspended`
- [ ] Remove direct task creation from statement handlers
- [ ] Manage `ast_path` advancement in statement handlers only

### Phase 5: Remove Old Code & Optimize Persistence

- [ ] Create database migration to drop `awaiting_task_id` column
- [ ] Remove `handle_awaiting_task()` function
- [ ] Remove all intermediate DB writes that track progress
- [ ] Keep only suspension and completion writes:
  - [ ] On suspension: save `ast_path`, `locals` with `__suspended_task`
  - [ ] On completion: save result and mark status
- [ ] Remove scattered task status checking code
- [ ] Update all code that references `awaiting_task_id` to use `__suspended_task` instead

### Phase 6: Future Enhancements (Post-Refactor)

- [ ] Implement `Task.delay()` support
- [ ] Implement `Task.any()` support
- [ ] Implement `Task.race()` support
- [ ] Add try-catch for error handling
- [ ] Allow failed tasks to return error values instead of terminating workflow
- [ ] Add support for cancellation

## Migration Path

### Database Changes

1. **Migration 5** (Created): Rename `statement_path` to `ast_path`
2. **Future Migration**: Drop `awaiting_task_id` column (Phase 5)

### Backwards Compatibility

**None.** This is a breaking change. All existing workflows will need to be re-executed from the beginning after this refactor is deployed. We are not maintaining backwards compatibility.

## Testing Strategy

After each phase:

1. Run existing workflow test suite
2. Add new tests for new functionality
3. Ensure suspension/resumption works correctly
4. Verify idempotent re-execution

Focus tests on:
- Simple expressions
- Nested expressions
- Task suspension and resumption
- Composed tasks (`Task.all()`, etc.)
- Error cases

## Benefits

### Developer Experience

- **Clearer code structure** - Obvious separation between expression evaluation and statement execution
- **Easier to reason about** - Expression evaluation is pure, side effects are explicit
- **Easier to test** - Can test expression evaluation in isolation
- **Easier to extend** - Adding new expression types is straightforward

### Performance

- **Fewer DB queries** - No intermediate progress tracking
- **Faster execution** - In-memory evaluation until suspension
- **Simpler queries** - No complex joins or status checks per statement

### Correctness

- **Idempotent resumption** - Safe to retry failed executions
- **No duplicate tasks** - Can't accidentally create the same task twice
- **Proper task composition** - Naturally handles complex task dependencies

## Implementation Details

### Await Expression Evaluation Flow

**First execution (no suspended task exists):**
1. Check if `locals.__suspended_task` exists → NO
2. Evaluate Task expression (e.g., `Task.run("foo", {x: 1})`)
3. Create execution record in DB
4. Store task structure in `locals.__suspended_task`
5. Return `Suspended(task_id)`

**Resumption (suspended task exists):**
1. Check if `locals.__suspended_task` exists → YES
2. Query task status from DB (use task_id from structure)
3. If complete: clear `__suspended_task`, return `Value(result)`
4. If still pending: return `Suspended(task_id)`

This makes re-evaluation idempotent - we never create duplicate tasks.

### Composed Task Resolution

For `Task.all([task1, task2, ...])`:
```
1. Check __suspended_task structure
2. For each child task (recursively):
   - Query status from DB
   - If any incomplete → return Suspended
3. If all complete → return Value([result1, result2, ...])
```

For `Task.any([task1, task2, ...])`:
```
1. Check __suspended_task structure
2. For each child task (recursively):
   - Query status from DB
   - If any complete → return Value(first_completed_result)
3. If all still pending → return Suspended
```

For `Task.race([task1, task2, ...])`:
```
1. Check __suspended_task structure
2. For each child task (recursively):
   - Query status from DB
   - If any succeeded → return Value(result)
3. If all failed → return Value(error)
4. If some pending → return Suspended
```

### Statement Execution Rules

**ast_path management:**
- Only statement handlers manage `ast_path`
- Expression evaluation NEVER modifies `ast_path`
- When expression returns `Suspended`: persist current `ast_path` (unchanged)
- When expression returns `Value`: statement may advance `ast_path` (if appropriate)

**Example - Assignment statement:**
```rust
let result = evaluate_expression(&statement["right"], &locals);
match result {
    Value(v) => {
        assign_variable(&mut locals, var_name, v, depth);
        let next_path = advance_path(&ast_path);
        // Write to DB with next_path
    }
    Suspended(task_id) => {
        // Write to DB with current ast_path (unchanged)
        return StepResult::Suspended;
    }
}
```

### Database Write Strategy

**Only write to DB in these cases:**

1. **On suspension** (expression returns `Suspended`):
   ```sql
   UPDATE workflow_execution_context
   SET ast_path = $1,  -- Current path (not advanced)
       locals = $2     -- Contains __suspended_task
   WHERE execution_id = $3
   ```

2. **On completion** (workflow finishes):
   ```sql
   UPDATE executions
   SET status = 'completed',
       result = $1
   WHERE id = $2
   ```

**Remove all intermediate writes:**
- ❌ No writes when advancing between statements
- ❌ No writes when entering/exiting scopes
- ❌ No writes when assigning variables
- ❌ No writes when iterating loops

The executor runs in-memory until it suspends or completes.

### Suspended Task Lifecycle

**Creation:**
```javascript
// Workflow code
let x = await Task.run("foo", {})
```

**After first evaluation (suspended):**
```json
{
  "locals": {
    "scope_stack": [...],
    "__suspended_task": {
      "type": "run",
      "task_id": "uuid-123"
    }
  }
}
```

**After task completes (resumed):**
```json
{
  "locals": {
    "scope_stack": [
      {
        "variables": {
          "x": {"result": "from task"}
        }
      }
    ]
    // __suspended_task is cleared
  }
}
```

### Edge Cases to Handle

1. **Multiple awaits in sequence:**
   - Each await creates its own `__suspended_task`
   - When first completes, clear it and advance `ast_path`
   - Next await creates new `__suspended_task`

2. **Await inside loop:**
   - Each iteration may suspend at same `ast_path`
   - Loop iteration counter in scope helps distinguish
   - `__suspended_task` is per-iteration

3. **Nested composed tasks:**
   - Recurse through structure checking status
   - Store entire nested structure in `__suspended_task`

4. **Task creation with variables:**
   - Resolve variables BEFORE creating task
   - Store resolved inputs in execution record
   - Don't re-resolve on resumption

## Open Questions

- [ ] Should `Task.delay()` create an execution record or use a different mechanism?
- [ ] How to handle timeouts on composed tasks?
- [ ] Should we add a `Task.timeout()` combinator?
- [ ] What should the error value structure look like for failed tasks?

## References

- Original design discussion: [Previous conversation summary]
- Current executor implementation: `core/src/interpreter/executor.rs`
- Path navigation helpers: Lines 20-91 in executor.rs
- Scope stack implementation: Lines 180-410 in executor.rs
