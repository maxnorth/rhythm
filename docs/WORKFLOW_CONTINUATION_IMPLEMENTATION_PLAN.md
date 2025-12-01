# Workflow Continuation Implementation Plan

This document outlines the step-by-step plan to implement event-driven workflow continuation as described in [WORKFLOW_CONTINUATION_DESIGN.md](WORKFLOW_CONTINUATION_DESIGN.md).

## Overview

We're implementing:
1. Separate `timer_tasks` table for delays
2. `builtin.resume_workflow` task type
3. Task completion triggers resume tasks
4. Timer expiration triggers resume tasks
5. Background timer processor with lease

## Prerequisites

- Database migration system in place
- Background job infrastructure (lease-based)
- Workflow executor exists and can suspend/resume

## Implementation Phases

### Phase 1: Core Infrastructure (MVP)

**Goal:** Get basic task completion → workflow resume working

#### Step 1.1: Database Schema

**File:** `core/migrations/YYYYMMDD_workflow_continuation.sql`

```sql
-- Timer tasks table
CREATE TABLE timer_tasks (
    id TEXT PRIMARY KEY,
    fire_at TIMESTAMPTZ NOT NULL,
    workflow_id TEXT NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
    timer_type TEXT NOT NULL DEFAULT 'delay',
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_timer_tasks_fire_at
ON timer_tasks(fire_at)
WHERE fire_at IS NOT NULL;

CREATE INDEX idx_timer_tasks_workflow_id
ON timer_tasks(workflow_id);
```

**Validation:**
- Run migration
- Verify indexes created
- Test CASCADE delete

---

#### Step 1.2: Update Task Completion to Enqueue Resume Tasks

**File:** `core/src/executions/lifecycle.rs`

**Changes:**

```rust
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
    .await
    .context("Failed to complete execution and enqueue resume task")?;

    Ok(())
}
```

**Tests:**
```rust
#[tokio::test(flavor = "multi_thread")]
async fn test_task_completion_enqueues_resume_task() {
    let _guard = with_test_db().await;

    // Create workflow
    let workflow_id = create_test_workflow().await.unwrap();

    // Create task with parent
    let task_id = create_execution(CreateExecutionParams {
        id: Some("test-task".to_string()),
        parent_workflow_id: Some(workflow_id.clone()),
        exec_type: ExecutionType::Task,
        target_name: "test.task".to_string(),
        queue: "default".to_string(),
        // ... other fields
    }).await.unwrap();

    // Complete task
    complete_execution(&task_id, json!({"result": 42})).await.unwrap();

    // Verify resume task was created
    let resume_tasks: Vec<Execution> = sqlx::query_as(
        "SELECT * FROM executions
         WHERE target_name = 'builtin.resume_workflow'
         AND args->>0 = $1"
    )
    .bind(&workflow_id)
    .fetch_all(get_pool().await.unwrap().as_ref())
    .await
    .unwrap();

    assert_eq!(resume_tasks.len(), 1);
    assert_eq!(resume_tasks[0].status, ExecutionStatus::Pending);
    assert_eq!(resume_tasks[0].queue, "system");
    assert_eq!(resume_tasks[0].priority, 10);
}
```

**Validation:**
- Test passes
- Resume task appears in database
- Transaction is atomic (both happen or neither)

---

#### Step 1.3: Implement Resume Workflow Handler

**File:** `core/src/worker.rs` (or new `core/src/builtins/resume_workflow.rs`)

**Changes:**

```rust
// In worker task execution loop, add handler for builtin functions

async fn execute_task(execution: &Execution) -> Result<()> {
    // Check if this is a builtin function
    if execution.target_name.starts_with("builtin.") {
        return execute_builtin_task(execution).await;
    }

    // Normal task execution
    // ... existing code
}

async fn execute_builtin_task(execution: &Execution) -> Result<()> {
    match execution.target_name.as_str() {
        "builtin.resume_workflow" => {
            let workflow_id = execution.args[0].as_str()
                .ok_or_else(|| anyhow::anyhow!("resume_workflow requires workflow_id"))?;

            // Execute workflow step
            execute_workflow_step(workflow_id).await?;

            Ok(())
        }
        _ => Err(anyhow::anyhow!("Unknown builtin function: {}", execution.target_name))
    }
}
```

**Tests:**
```rust
#[tokio::test(flavor = "multi_thread")]
async fn test_resume_workflow_builtin() {
    let _guard = with_test_db().await;

    // Create suspended workflow
    let workflow_id = create_suspended_workflow().await.unwrap();

    // Create resume task
    let resume_task_id = create_execution(CreateExecutionParams {
        id: Some("resume-task".to_string()),
        exec_type: ExecutionType::Task,
        target_name: "builtin.resume_workflow".to_string(),
        queue: "system".to_string(),
        args: json!([workflow_id]),
        // ... other fields
    }).await.unwrap();

    // Execute resume task
    let execution = get_execution(&resume_task_id).await.unwrap();
    execute_builtin_task(&execution).await.unwrap();

    // Verify workflow was resumed (attempted execution)
    // This depends on what execute_workflow_step returns
}
```

**Validation:**
- Resume task handler works
- Calls `execute_workflow_step`
- Handles errors gracefully

---

#### Step 1.4: Update Workflow Executor to Handle Resume

**File:** `core/src/interpreter/executor.rs`

**Changes:**

Ensure `execute_workflow_step` can handle being called when workflow isn't ready:

```rust
pub async fn execute_workflow_step(execution_id: &str) -> Result<StepResult> {
    let pool = get_pool().await?;

    // Load context
    let (workflow_def_id, statement_index, mut locals, awaiting_task_id) =
        load_workflow_context(execution_id).await?;

    // If we're awaiting a task, check if it's done
    if let Some(task_id) = awaiting_task_id {
        let task_status = get_task_status(&task_id).await?;

        match task_status {
            ExecutionStatus::Completed => {
                // Task done - get result and continue
                let task_result = get_task_result(&task_id).await?;

                // Assign to variable if specified
                if let Some(var_name) = get_current_statement_assign_var(&locals, statement_index) {
                    assign_variable(&mut locals, var_name, task_result);
                }

                // Clear awaiting state
                clear_awaiting_task(execution_id).await?;

                // Continue to next statement (fall through to execution below)
            }
            ExecutionStatus::Failed => {
                // Task failed
                let error = get_task_error(&task_id).await?;
                fail_workflow(execution_id, error).await?;
                return Ok(StepResult::Completed);
            }
            _ => {
                // Task still running - can't progress
                return Ok(StepResult::Suspended);
            }
        }
    }

    // Continue workflow execution from current statement
    // ... existing code
}
```

**Tests:**
```rust
#[tokio::test(flavor = "multi_thread")]
async fn test_workflow_resume_when_task_complete() {
    let _guard = with_test_db().await;

    // Create workflow awaiting a task
    let (workflow_id, task_id) = create_workflow_awaiting_task().await.unwrap();

    // Task is not complete yet
    let result = execute_workflow_step(&workflow_id).await.unwrap();
    assert_eq!(result, StepResult::Suspended);

    // Complete the task
    complete_execution(&task_id, json!({"value": 42})).await.unwrap();

    // Now workflow can progress
    let result = execute_workflow_step(&workflow_id).await.unwrap();
    assert_eq!(result, StepResult::Continue); // or Completed, depending on workflow
}
```

**Validation:**
- Workflow resumes when task complete
- Workflow stays suspended when task not ready
- Task result assigned to correct variable

---

### Phase 2: Timer Support

**Goal:** Implement `Task.delay()` using timer tasks

#### Step 2.1: Implement Task.delay() Parsing

**File:** `core/src/interpreter/executor.rs`

**Changes:**

```rust
// In function call handler
"delay" if parent == "Task" => {
    let seconds = args[0].as_i64()
        .ok_or_else(|| anyhow::anyhow!("Task.delay requires numeric argument"))?;

    // Create timer task
    let timer_id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO timer_tasks (id, fire_at, workflow_id, timer_type)
        VALUES ($1, NOW() + ($2 || ' seconds')::INTERVAL, $3, 'delay')
        "#
    )
    .bind(&timer_id)
    .bind(seconds)
    .bind(execution_id)
    .execute(pool.as_ref())
    .await?;

    // Return timer ID (workflow will await it like a task)
    return Ok(json!(timer_id));
}
```

**Tests:**
```rust
#[tokio::test(flavor = "multi_thread")]
async fn test_task_delay_creates_timer() {
    let _guard = with_test_db().await;

    // Execute workflow with delay
    let workflow = parse_workflow("
        workflow test() {
            timer = Task.delay(30)
            return timer
        }
    ").unwrap();

    let workflow_id = execute_workflow(&workflow).await.unwrap();

    // Verify timer was created
    let timers: Vec<TimerTask> = sqlx::query_as(
        "SELECT * FROM timer_tasks WHERE workflow_id = $1"
    )
    .bind(&workflow_id)
    .fetch_all(get_pool().await.unwrap().as_ref())
    .await
    .unwrap();

    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].timer_type, "delay");
}
```

---

#### Step 2.2: Implement Timer Processor Background Job

**File:** `core/src/timers.rs` (new file)

```rust
use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::time::Duration;

/// Process expired timers and enqueue resume tasks
pub async fn process_expired_timers(pool: Arc<PgPool>) -> Result<usize> {
    // Delete expired timers and get workflow IDs
    let expired: Vec<(String, String)> = sqlx::query_as(
        r#"
        DELETE FROM timer_tasks
        WHERE fire_at <= NOW()
        RETURNING id, workflow_id
        "#
    )
    .fetch_all(pool.as_ref())
    .await?;

    // Enqueue resume tasks for each workflow
    for (timer_id, workflow_id) in &expired {
        sqlx::query(
            r#"
            INSERT INTO executions (id, type, target_name, queue, status, args, kwargs, priority, max_retries)
            VALUES (gen_random_uuid()::text, 'task', 'builtin.resume_workflow', 'system', 'pending', $1, '{}', 10, 0)
            "#
        )
        .bind(json!([workflow_id]))
        .execute(pool.as_ref())
        .await?;
    }

    Ok(expired.len())
}

/// Start timer processor background job with lease
pub async fn start_timer_processor(pool: Arc<PgPool>) {
    // Use lease-based background job (see BACKGROUND_JOBS_DESIGN.md)
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;

            match process_expired_timers(pool.clone()).await {
                Ok(count) if count > 0 => {
                    eprintln!("Processed {} expired timers", count);
                }
                Err(e) => {
                    eprintln!("Error processing timers: {}", e);
                }
                _ => {}
            }
        }
    });
}
```

**File:** `core/src/lib.rs`

Add module:
```rust
pub mod timers;
```

**File:** `core/src/bin/main.rs` or worker initialization

Start timer processor:
```rust
// After database initialization
timers::start_timer_processor(pool.clone()).await;
```

**Tests:**
```rust
#[tokio::test(flavor = "multi_thread")]
async fn test_timer_processor_resumes_workflow() {
    let _guard = with_test_db().await;

    // Create workflow
    let workflow_id = create_test_workflow().await.unwrap();

    // Create timer that expires in 1 second
    sqlx::query(
        "INSERT INTO timer_tasks (id, fire_at, workflow_id, timer_type)
         VALUES ('timer-1', NOW() + INTERVAL '1 second', $1, 'delay')"
    )
    .bind(&workflow_id)
    .execute(get_pool().await.unwrap().as_ref())
    .await
    .unwrap();

    // Wait for timer to expire
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Process timers
    let count = process_expired_timers(get_pool().await.unwrap()).await.unwrap();
    assert_eq!(count, 1);

    // Verify resume task was created
    let resume_tasks: Vec<Execution> = sqlx::query_as(
        "SELECT * FROM executions
         WHERE target_name = 'builtin.resume_workflow'
         AND args->>0 = $1"
    )
    .bind(&workflow_id)
    .fetch_all(get_pool().await.unwrap().as_ref())
    .await
    .unwrap();

    assert_eq!(resume_tasks.len(), 1);
}
```

---

#### Step 2.3: Update Workflow Executor to Check Timer Completion

**File:** `core/src/interpreter/executor.rs`

**Changes:**

```rust
async fn get_task_status(task_id: &str) -> Result<ExecutionStatus> {
    let pool = get_pool().await?;

    // Check if it's an execution
    if let Some(status) = sqlx::query_scalar::<_, String>(
        "SELECT status FROM executions WHERE id = $1"
    )
    .bind(task_id)
    .fetch_optional(pool.as_ref())
    .await? {
        return Ok(status.parse()?);
    }

    // Check if it's a timer (if timer doesn't exist, it fired)
    let timer_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM timer_tasks WHERE id = $1)"
    )
    .bind(task_id)
    .fetch_one(pool.as_ref())
    .await?;

    if !timer_exists {
        // Timer was deleted = it fired = completed
        return Ok(ExecutionStatus::Completed);
    }

    // Timer still exists = pending
    Ok(ExecutionStatus::Pending)
}
```

**Tests:**
```rust
#[tokio::test(flavor = "multi_thread")]
async fn test_expired_timer_treated_as_complete() {
    let _guard = with_test_db().await;

    let timer_id = "timer-123";

    // Create timer
    sqlx::query(
        "INSERT INTO timer_tasks (id, fire_at, workflow_id)
         VALUES ($1, NOW() - INTERVAL '1 second', 'wf-1')"
    )
    .bind(timer_id)
    .execute(get_pool().await.unwrap().as_ref())
    .await
    .unwrap();

    // Delete it (simulate expiration)
    sqlx::query("DELETE FROM timer_tasks WHERE id = $1")
        .bind(timer_id)
        .execute(get_pool().await.unwrap().as_ref())
        .await
        .unwrap();

    // Check status
    let status = get_task_status(timer_id).await.unwrap();
    assert_eq!(status, ExecutionStatus::Completed);
}
```

---

### Phase 3: Task.all() and Task.any()

**Goal:** Support multiple parallel tasks/timers

#### Step 3.1: Implement Task.all() Parsing

**File:** `core/src/interpreter/executor.rs`

```rust
"all" if parent == "Task" => {
    let task_ids = args[0].as_array()
        .ok_or_else(|| anyhow::anyhow!("Task.all requires array argument"))?
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect::<Vec<_>>();

    // Store in current statement that we're waiting for all these tasks
    // The checkpoint/locals already contains the parsed statement with this info

    // Set workflow awaiting all these tasks
    sqlx::query(
        "UPDATE workflow_execution_context
         SET awaiting_task_id = $1  -- Use first one for simple tracking
         WHERE execution_id = $2"
    )
    .bind(&task_ids[0])
    .bind(execution_id)
    .execute(pool.as_ref())
    .await?;

    // Return suspended - workflow will check all tasks when resumed
    return Ok(json!({
        "type": "await_all",
        "task_ids": task_ids
    }));
}
```

**Note:** The workflow's parsed statement already contains the full list of task IDs. The executor just needs to check them all when it resumes.

---

#### Step 3.2: Update Executor to Check Multiple Tasks

**File:** `core/src/interpreter/executor.rs`

```rust
// When resuming after await Task.all([...])
async fn check_all_tasks_complete(task_ids: &[String]) -> Result<bool> {
    for task_id in task_ids {
        let status = get_task_status(task_id).await?;
        if status != ExecutionStatus::Completed {
            return Ok(false);  // At least one not done
        }
    }
    Ok(true)  // All done
}

async fn fetch_all_results(task_ids: &[String]) -> Result<Vec<JsonValue>> {
    let mut results = Vec::new();
    for task_id in task_ids {
        let result = get_task_result(task_id).await?;
        results.push(result);
    }
    Ok(results)
}

// In execute_workflow_step, when handling await statement:
if statement.get("type") == Some("await_all") {
    let task_ids = statement.get("task_ids").unwrap().as_array().unwrap();

    if check_all_tasks_complete(task_ids).await? {
        // All done!
        let results = fetch_all_results(task_ids).await?;

        if let Some(var_name) = statement.get("assign_to") {
            assign_variable(&mut locals, var_name, json!(results));
        }

        // Continue to next statement
    } else {
        // Not all done yet
        return Ok(StepResult::Suspended);
    }
}
```

---

#### Step 3.3: Implement Task.any() and Task.race()

Similar pattern to Task.all(), but:
- `Task.any()`: Check if at least one successful completion
- `Task.race()`: Check if any terminal state (success or failure)

---

### Phase 4: Integration Testing

#### Test 4.1: End-to-End Workflow with Task

```rust
#[tokio::test(flavor = "multi_thread")]
async fn test_workflow_with_task_e2e() {
    let _guard = with_test_db().await;

    // Register workflow
    register_workflow("
        workflow process_order(inputs) {
            result = await Task.run('validate_order', inputs)
            return result
        }
    ").await.unwrap();

    // Start workflow
    let workflow_id = start_workflow("process_order", json!({"order_id": 123}))
        .await.unwrap();

    // Workflow should be suspended
    let status = get_execution_status(&workflow_id).await.unwrap();
    assert_eq!(status, ExecutionStatus::Suspended);

    // Find the task it created
    let task = find_child_task(&workflow_id).await.unwrap();

    // Complete the task
    complete_execution(&task.id, json!({"valid": true})).await.unwrap();

    // Resume task should be enqueued
    let resume_task = find_resume_task(&workflow_id).await.unwrap();

    // Execute resume task
    let worker = Worker::new(...);
    worker.execute_task(&resume_task).await.unwrap();

    // Workflow should be completed
    let status = get_execution_status(&workflow_id).await.unwrap();
    assert_eq!(status, ExecutionStatus::Completed);

    // Check result
    let result = get_workflow_result(&workflow_id).await.unwrap();
    assert_eq!(result["valid"], true);
}
```

#### Test 4.2: End-to-End Workflow with Delay

```rust
#[tokio::test(flavor = "multi_thread")]
async fn test_workflow_with_delay_e2e() {
    let _guard = with_test_db().await;

    register_workflow("
        workflow delayed_process(inputs) {
            await Task.delay(1)
            return {done: true}
        }
    ").await.unwrap();

    let workflow_id = start_workflow("delayed_process", json!({}))
        .await.unwrap();

    // Should be suspended
    assert_eq!(get_execution_status(&workflow_id).await.unwrap(),
               ExecutionStatus::Suspended);

    // Wait for timer to expire
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Process timers
    process_expired_timers(get_pool().await.unwrap()).await.unwrap();

    // Resume task should exist
    let resume_task = find_resume_task(&workflow_id).await.unwrap();

    // Execute it
    let worker = Worker::new(...);
    worker.execute_task(&resume_task).await.unwrap();

    // Workflow should complete
    assert_eq!(get_execution_status(&workflow_id).await.unwrap(),
               ExecutionStatus::Completed);
}
```

#### Test 4.3: Task.all() with Mixed Tasks and Timers

```rust
#[tokio::test(flavor = "multi_thread")]
async fn test_task_all_mixed() {
    let _guard = with_test_db().await;

    register_workflow("
        workflow parallel_work(inputs) {
            task1 = Task.run('work1', {})
            task2 = Task.run('work2', {})
            timer = Task.delay(1)

            results = await Task.all([task1, task2, timer])
            return results
        }
    ").await.unwrap();

    let workflow_id = start_workflow("parallel_work", json!({}))
        .await.unwrap();

    // Complete tasks
    let tasks = find_child_tasks(&workflow_id).await.unwrap();
    complete_execution(&tasks[0].id, json!({"result": 1})).await.unwrap();
    complete_execution(&tasks[1].id, json!({"result": 2})).await.unwrap();

    // Wait for timer
    tokio::time::sleep(Duration::from_millis(1500)).await;
    process_expired_timers(get_pool().await.unwrap()).await.unwrap();

    // Process all resume tasks
    let resume_tasks = find_all_resume_tasks(&workflow_id).await.unwrap();
    for rt in resume_tasks {
        Worker::new(...).execute_task(&rt).await.unwrap();
    }

    // Workflow should complete with all results
    assert_eq!(get_execution_status(&workflow_id).await.unwrap(),
               ExecutionStatus::Completed);

    let result = get_workflow_result(&workflow_id).await.unwrap();
    assert_eq!(result.as_array().unwrap().len(), 3);
}
```

---

## Rollout Plan

### Week 1: Core Infrastructure
- [ ] Database migration
- [ ] Update `complete_execution()`
- [ ] Implement `builtin.resume_workflow` handler
- [ ] Update workflow executor to handle resume
- [ ] Unit tests for each component

### Week 2: Timer Support
- [ ] Implement `Task.delay()` parsing
- [ ] Create timer processor background job
- [ ] Integrate with lease system (from BACKGROUND_JOBS_DESIGN.md)
- [ ] Update executor to check timer completion
- [ ] Timer tests

### Week 3: Multi-Task Support
- [ ] Implement `Task.all()` parsing
- [ ] Update executor to check multiple tasks
- [ ] Implement `Task.any()` and `Task.race()`
- [ ] Multi-task tests

### Week 4: Integration & Polish
- [ ] End-to-end tests
- [ ] Performance testing
- [ ] Documentation updates
- [ ] Monitor and optimize

---

## Testing Strategy

### Unit Tests
- Each component tested in isolation
- Mock dependencies where needed
- Test edge cases (failures, timeouts, etc.)

### Integration Tests
- Full workflow execution flows
- Task completion → resume → continuation
- Timer expiration → resume → continuation
- Multiple parallel tasks/timers

### Load Tests
- 100 concurrent workflows
- 1000 parallel tasks in Task.all()
- Long-running timers (hours/days)
- Failure scenarios (crashed workers, database issues)

---

## Monitoring & Observability

### Metrics to Track
- Resume tasks created per second
- Resume tasks processed per second
- Workflow resume latency (task complete → workflow continues)
- Timer processing latency (expire time → resume task created)
- Stuck workflows (suspended > 1 hour with no activity)

### Alerts
- Resume tasks building up in queue
- Timer processor not running
- Workflows stuck in suspended state
- High resume task failure rate

---

## Migration Strategy

### Backward Compatibility

**Existing workflows must continue to work during rollout.**

1. **Phase 0: Add schema, don't use yet**
   - Deploy migration
   - Old code still works
   - New tables exist but empty

2. **Phase 1: New workflows use new system**
   - New workflow executions use resume tasks
   - Old in-flight workflows use old mechanism
   - Both systems coexist

3. **Phase 2: Migrate old workflows (optional)**
   - Can leave old workflows to complete naturally
   - Or: write migration script to update them

### Rollback Plan

If issues found:
1. Stop creating resume tasks (feature flag)
2. Revert to old workflow execution path
3. Clean up orphaned resume tasks
4. Keep timer_tasks table (no harm)

---

## Success Criteria

- [ ] Workflows resume immediately after task completion (< 1s latency)
- [ ] Timers fire within 1s of expiration time
- [ ] Task.all() works with 100+ parallel tasks
- [ ] No workflow gets stuck in suspended state
- [ ] All tests pass
- [ ] Performance benchmark: 1000 workflows/sec throughput

---

## Open Questions

1. **Resume task deduplication:** Start without it or implement from day 1?
   - **Decision:** Start without, add if profiling shows need

2. **Timer processor frequency:** 100ms, 1s, or 5s?
   - **Decision:** Start with 1s, make configurable

3. **System queue priority:** Should resume tasks jump ahead of user tasks?
   - **Decision:** Yes, use priority 10 (high) on system queue

4. **Lease for timer processor:** Use existing lease infrastructure or build new?
   - **Decision:** Use lease system from BACKGROUND_JOBS_DESIGN.md

5. **Fire-and-forget optimization:** Track which tasks don't need resume?
   - **Decision:** No optimization initially, resume is cheap enough

---

## Next Steps

1. Review this plan with team
2. Create tickets for each phase
3. Start with Phase 1, Step 1.1 (database migration)
4. Implement incrementally, testing each step
5. Deploy to staging environment
6. Monitor and iterate

---

## References

- [WORKFLOW_CONTINUATION_DESIGN.md](WORKFLOW_CONTINUATION_DESIGN.md) - Full design
- [BACKGROUND_JOBS_DESIGN.md](BACKGROUND_JOBS_DESIGN.md) - Lease-based background jobs
- [Temporal Architecture](https://github.com/temporalio/temporal) - Inspiration and validation
