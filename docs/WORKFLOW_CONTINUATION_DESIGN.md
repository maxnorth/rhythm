# Workflow Continuation Design

This document outlines how workflows resume execution after awaiting tasks, timers, and signals.

## Current Status

**Not yet implemented.** This is design documentation for future implementation.

## The Problem

When a workflow awaits a task or timer:

```javascript
workflow example(inputs) {
  result = await Task.run("process_data", inputs)
  // Workflow needs to resume here when task completes
  return result
}
```

**Questions:**
1. How does the workflow know when the task is complete?
2. How does it resume execution at the right point?
3. How do we handle `Task.all()` and `Task.any()` with multiple items?
4. What about timers/delays mixed with tasks?

## Design Principles

### 1. Event-Driven, Not Polling

**Bad:** Workers repeatedly check if workflows can continue
```rust
// ❌ Don't do this
loop {
    for workflow in get_suspended_workflows() {
        if can_progress(workflow) {
            execute(workflow);
        }
    }
}
```

**Good:** Task/timer completion triggers workflow resume
```rust
// ✅ Do this
when task_completes(task_id) {
    enqueue_resume_task(parent_workflow_id);
}
```

### 2. Workflows Are State, Not Queue Items

**Key insight:** Suspended workflows are just state records, not tasks in the queue.

- Workflows have `status = 'suspended'` in the database
- They do NOT become `status = 'pending'` when ready
- Instead, we enqueue a **separate task** to check and resume them

### 3. Resume Tasks Are Explicit

We introduce a new built-in task type: `builtin.resume_workflow`

```sql
INSERT INTO executions (id, type, target_name, queue, status, args, ...)
VALUES (
    'resume-abc',
    'task',
    'builtin.resume_workflow',
    'system',
    'pending',
    '["workflow-123"]',  -- workflow_id as argument
    ...
);
```

This task:
- Goes through the normal task queue
- Has high priority (system queue)
- Checks if workflow can progress
- Executes workflow if ready, or no-ops if not

## Architecture

### Schema Changes

#### Timer Tasks Table

```sql
CREATE TABLE timer_tasks (
    id TEXT PRIMARY KEY,
    fire_at TIMESTAMPTZ NOT NULL,
    workflow_id TEXT NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
    timer_type TEXT NOT NULL,  -- 'delay', 'timeout', etc.
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_timer_tasks_fire_at
ON timer_tasks(fire_at);

CREATE INDEX idx_timer_tasks_workflow_id
ON timer_tasks(workflow_id);
```

#### No Schema Changes to workflow_execution_context

**Key insight:** We don't need to store what tasks the workflow is waiting for in the database.

- The workflow's checkpoint (its execution state/locals) already contains this information
- When the workflow resumes, it knows from its current statement what it's awaiting
- We just use `parent_workflow_id` on tasks to trigger resume

## Implementation

### 1. Task Completion Triggers Resume

When a task completes, atomically mark it complete and enqueue a resume task for the parent workflow:

```rust
// executions/lifecycle.rs

pub async fn complete_execution(execution_id: &str, result: JsonValue) -> Result<()> {
    let pool = get_pool().await?;

    // Single atomic query: mark complete + enqueue resume task
    sqlx::query(
        r#"
        WITH completed_task AS (
            UPDATE executions
            SET status = 'completed',
                result = $1,
                completed_at = NOW()
            WHERE id = $2
            RETURNING parent_workflow_id
        )
        INSERT INTO executions (id, type, target_name, queue, status, args, kwargs, priority, max_retries)
        SELECT
            gen_random_uuid()::text,
            'task',
            'builtin.resume_workflow',
            'system',
            'pending',
            jsonb_build_array(parent_workflow_id),
            '{}'::jsonb,
            10,
            0
        FROM completed_task
        WHERE parent_workflow_id IS NOT NULL
        "#
    )
    .bind(&result)
    .bind(execution_id)
    .execute(pool.as_ref())
    .await?;

    Ok(())
}
```

**Key points:**
- ✅ Single atomic query (both happen or neither)
- ✅ Uses `parent_workflow_id` from task - no need to query workflow context
- ✅ If task has no parent, no resume task is created (fire-and-forget)
- ✅ Don't check if workflow is actually waiting - just enqueue resume task
- ✅ Workflow executor will check its own state when resume task runs

### 2. Timer Expiration Triggers Resume

Background job with lease processes expired timers:

```rust
// Background job (runs with lease - see BACKGROUND_JOBS_DESIGN.md)

async fn process_expired_timers(pool: Arc<PgPool>) -> Result<usize> {
    // Delete expired timers
    let expired: Vec<(String, String)> = sqlx::query_as(
        r#"
        DELETE FROM timer_tasks
        WHERE fire_at <= NOW()
        RETURNING id, workflow_id
        "#
    )
    .fetch_all(pool.as_ref())
    .await?;

    // Enqueue resume tasks
    for (timer_id, workflow_id) in &expired {
        enqueue_resume_workflow_task(workflow_id).await?;
    }

    Ok(expired.len())
}
```

### 3. Worker Handles Resume Tasks

```rust
// worker.rs - in task execution loop

if execution.target_name == "builtin.resume_workflow" {
    let workflow_id = execution.args[0].as_str()
        .ok_or_else(|| anyhow::anyhow!("resume_workflow requires workflow_id"))?;

    // Just try to execute the workflow
    // The workflow executor will check its own state to see if it can progress
    execute_workflow_step(workflow_id).await?;

    return Ok(());
}
```

**That's it!** The workflow executor already:
- Knows what statement it's on
- Knows what it's waiting for (from its checkpoint/execution state)
- Can check if awaited items are complete
- Will either continue or return `Suspended` if not ready yet

### 4. Workflow Executor Checks Its Own State

When resume task executes the workflow, the executor checks what it's waiting for:

```rust
// interpreter/executor.rs

pub async fn execute_workflow_step(execution_id: &str) -> Result<StepResult> {
    let pool = get_pool().await?;

    // Load context (includes locals/checkpoint with execution state)
    let (workflow_def_id, statement_index, mut locals, awaiting_task_id) =
        load_workflow_context(execution_id).await?;

    // If we were awaiting a task, check if it's done
    if let Some(task_id) = awaiting_task_id {
        let task_status = get_task_status(&task_id).await?;

        match task_status {
            TaskStatus::Completed(result) => {
                // Task is done! Extract result and continue
                if let Some(var_name) = get_current_statement_assign_var() {
                    assign_variable(&mut locals, var_name, result);
                }
                // Clear awaiting state
                clear_awaiting_task(execution_id).await?;
                // Continue to next statement
            }
            TaskStatus::Failed(error) => {
                // Task failed, handle error
                fail_workflow(execution_id, error).await?;
                return Ok(StepResult::Completed);
            }
            _ => {
                // Task still running - can't progress yet
                return Ok(StepResult::Suspended);
            }
        }
    }

    // Continue workflow execution from current statement
    // ...
}
```

**Key insight:** The workflow's `locals`/checkpoint already contains:
- What statement it's on
- What task ID it's waiting for (if any)
- For `Task.all()`, the list of task IDs is in the parsed statement

No need to store this separately in the database!

## Flow Diagrams

### Task.run() Flow

```
Workflow executes:
  result = await Task.run("process", inputs)
    ↓
1. Create task execution
    ↓
2. Set workflow status = 'suspended'
    ↓
4. Return StepResult::Suspended

[Time passes, worker processes task]

Task completes:
    ↓
1. Mark task status = 'completed'
    ↓
2. Find parent workflows (query workflow_execution_context)
    ↓
3. Enqueue resume task:
   INSERT INTO executions
   (target_name='builtin.resume_workflow', args='["workflow-123"]')
    ↓
Worker claims resume task:
    ↓
1. Load workflow context
    ↓
2. Check: are awaited tasks complete?
   - Yes! task-123 is completed
    ↓
3. Execute workflow step:
   - Fetch task result
   - Assign to variable 'result'
   - Clear awaiting state
   - Continue to next statement
```

### Task.all() Flow

```
Workflow executes:
  results = await Task.all([task1, task2, task3])
    ↓
1. Create 3 task executions (all with parent_workflow_id set)
    ↓
2. Workflow stores in its checkpoint:
   - Current statement has Task.all with [task1, task2, task3]
   - Statement index stays on this await
    ↓
3. Set workflow status = 'suspended'
    ↓
4. Return StepResult::Suspended

[Time passes]

Task1 completes:
    ↓
Enqueue resume task (based on parent_workflow_id)
    ↓
Worker picks up resume task:
    ↓
Execute workflow step:
   - Load current statement from checkpoint
   - See it's Task.all([task1, task2, task3])
   - Check: task1 done, task2 pending, task3 pending
   - Not all done yet
   - Return Suspended
    ↓
Workflow stays suspended

Task2 completes:
    ↓
Enqueue resume task
    ↓
Worker picks up resume task:
    ↓
Execute workflow step:
   - Check: task1 done, task2 done, task3 pending
   - Not all done yet
   - Return Suspended
    ↓
Workflow stays suspended

Task3 completes:
    ↓
Enqueue resume task
    ↓
Worker picks up resume task:
    ↓
Execute workflow step:
   - Check: task1 done, task2 done, task3 done
   - All done! ✓
   - Fetch all 3 results
   - Assign to variable 'results' as array
   - Advance to next statement
   - Continue workflow execution
```

**Note:** Workflow state (checkpoint) already contains the list `[task1, task2, task3]` from the parsed statement. No need to store separately!

### Task.delay() Flow

```
Workflow executes:
  await Task.delay(30)
    ↓
1. Create timer task:
   INSERT INTO timer_tasks
   (fire_at = NOW() + 30 seconds, workflow_id)
    ↓
2. Set workflow status = 'suspended'
    ↓
4. Return StepResult::Suspended

[30 seconds pass]

Background timer processor runs:
    ↓
1. Query: SELECT * FROM timer_tasks WHERE fire_at <= NOW()
   - Finds timer-123
    ↓
2. DELETE timer-123
    ↓
3. Enqueue resume task for workflow
    ↓
Worker picks up resume task:
    ↓
1. Load workflow context
    ↓
2. Check: timer-123 complete?
   - Timer doesn't exist in timer_tasks = expired = complete!
    ↓
3. Execute workflow step:
   - Clear awaiting state
   - Continue to next statement
```

## Task.any() and Task.race()

### Task.any()

Resolves when **any** awaited item completes successfully (ignores failures).

```javascript
fastest = await Task.any([
  Task.run("api1", inputs),
  Task.run("api2", inputs),
  Task.run("api3", inputs)
])
// Returns first successful result
```

**Implementation:**
- All completions (success or failure) trigger resume
- Workflow executor checks if at least one success available
- If only failures so far, returns Suspended (keeps waiting)
- Result: `{item_id: "task-abc", result: {...}}`
- The workflow's parsed AST already knows it's in `Task.any()` mode

**Note:** For simplicity, failed tasks also enqueue resume tasks. The workflow executor just checks and suspends again if no successful result yet. Can optimize later if needed.

### Task.race()

Resolves when **any** awaited item reaches terminal state (success OR failure).

```javascript
result = await Task.race([
  Task.run("operation", inputs),
  Task.delay(30)  // Timeout
])

if (result.item_id == timeout_timer) {
  return {error: "Operation timed out"}
}
```

**Implementation:**
- First completion (success or failure) triggers resume
- Result: `{item_id: "...", status: "completed"/"failed", result/error: ...}`
- The workflow's parsed AST already knows it's in `Task.race()` mode

## Optimization: Deduplication (Optional)

**Not implemented initially - optimize later if needed.**

Multiple resume tasks for the same workflow are fine:
- Resume task is cheap (just checks if workflow can progress)
- If workflow already progressed, resume task is a no-op
- Fire-and-forget tasks are rare
- Most workflows have < 10 parallel tasks

**If profiling shows this is a problem**, add deduplication:

```sql
INSERT INTO executions (...)
SELECT ...
WHERE NOT EXISTS (
    SELECT 1 FROM executions
    WHERE target_name = 'builtin.resume_workflow'
      AND args->>0 = $workflow_id
      AND status = 'pending'
)
```

**Trade-off:**
- Prevents redundant resume tasks
- Adds a subquery check on every enqueue
- Measure first, optimize if actually needed

## Edge Cases

### 1. Task Fails

If awaited task fails:
- For single task await: workflow resumes, gets error result
- For `Task.all()`: first failure triggers resume with error
- For `Task.any()`: failure ignored, wait for successful completion
- For `Task.race()`: first terminal state (including failure) triggers resume

### 2. Workflow Crashes During Execution

If workflow crashes while processing resume:
- Resume task fails and can be retried
- Workflow stays in suspended state
- Retry will reload context and try again

### 3. Timer Deleted Manually

If timer is deleted from `timer_tasks` table:
- Workflow will see it as "expired" (doesn't exist)
- Will resume as if timer fired
- This is acceptable behavior

### 4. Multiple Resume Tasks Enqueued

If 3 tasks complete before any resume task is processed:
- 3 resume tasks get enqueued
- First one processes and progresses workflow
- Other 2 complete as no-ops (workflow already progressed)
- This is inefficient but correct
- Deduplication optimization prevents this

## Performance Considerations

### Resume Task Overhead

**Question:** Is enqueuing a resume task for every completion expensive?

**Answer:** Not really:
- Resume task is just an INSERT (fast)
- Goes into high-priority 'system' queue
- Workers pick it up quickly
- Check is fast (query completion status)
- Most workflows have < 10 parallel tasks

**For 100 parallel tasks in Task.all():**
- Without deduplication: 100 resume tasks (99 no-ops)
- With deduplication: 1-2 resume tasks
- Deduplication recommended for heavy parallelism

### Timer Processor Frequency

Run timer processor every **100ms - 1 second**.

- More frequent = lower latency for short delays
- Less frequent = lower DB load
- 1 second is a good default (matches typical use cases)

## Testing Strategy

### Unit Tests

```rust
#[tokio::test]
async fn test_task_completion_enqueues_resume() {
    let _guard = with_test_db().await;

    // Create workflow
    let workflow_id = create_workflow(...).await;

    // Create task with parent
    let task_id = create_task_with_parent(workflow_id).await;

    // Set workflow awaiting this task
    set_awaiting_task(workflow_id, task_id).await;

    // Complete task
    complete_execution(&task_id, json!({"result": 42})).await.unwrap();

    // Check: resume task was enqueued
    let resume_tasks = find_resume_tasks(workflow_id).await;
    assert_eq!(resume_tasks.len(), 1);
    assert_eq!(resume_tasks[0].target_name, "builtin.resume_workflow");
}
```

### Integration Tests

Test full flows:
- Single task await
- Task.all with 3 tasks
- Task.any with racing tasks
- Task.delay
- Mixed task + timer in Task.all

## Migration Path

### Phase 1: Implement Core (MVP)

1. Add `timer_tasks` table
2. Implement `builtin.resume_workflow` handler
3. Update `complete_execution()` to enqueue resume tasks
5. Implement timer processor with lease
6. Support single task await

### Phase 2: Multi-Item Support

1. Implement Task.all()
2. Implement Task.any()
3. Support mixing tasks and timers

### Phase 3: Optimizations

1. Add deduplication for resume tasks
2. Tune timer processor frequency
3. Add metrics and monitoring

## Comparison with Temporal

| Aspect | Temporal | Rhythm |
|--------|----------|--------|
| **Timer storage** | Separate timer_tasks table | ✅ Same |
| **Resume trigger** | Transfer Task (queue item) | ✅ Resume task (queue item) |
| **Workflow state** | Suspended in history | ✅ Suspended in executions table |
| **Event-driven** | Yes (via Transfer Queue) | ✅ Yes (via resume tasks) |
| **Batching** | Multiple events → single workflow task | Natural (deduplication) |

**Our design aligns well with Temporal's proven architecture.**

## Future Enhancements

### Signals

Signals can use the same resume mechanism:

```javascript
approval = await wait_for_signal("approval")
```

When signal received:
- Store signal in `workflow_signals` table
- Enqueue resume task
- Worker checks if signal matches what workflow is waiting for
- If yes, continue execution

### Scheduled/Cron Jobs

Use same timer infrastructure:
```sql
INSERT INTO timer_tasks (fire_at, workflow_id, timer_type)
VALUES (next_cron_time(), 'cron-job-123', 'cron');
```

When timer fires:
- Execute scheduled workflow
- Calculate next execution time
- Insert new timer task

---

## Summary

**Key Design Decisions:**

1. ✅ **Workflows stay suspended** - never become 'pending' themselves
2. ✅ **Resume tasks are explicit** - `builtin.resume_workflow` goes through queue
3. ✅ **Event-driven** - task/timer completion triggers resume
4. ✅ **Simple** - check actual completion status, no complex counters
5. ✅ **Scalable** - works with 1 or 1000 parallel tasks
6. ✅ **Aligned with Temporal** - proven architecture pattern

**This design is production-ready and ready to implement.**
