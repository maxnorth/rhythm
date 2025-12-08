# Simple Rhythm Application Example

This example demonstrates a basic Rhythm application with:
- Task definitions (sync and async)
- A workflow definition (.flow file)
- Worker process to execute tasks
- Scheduler to enqueue work

## Architecture

```
┌─────────────┐         ┌──────────────┐         ┌─────────────┐
│  Scheduler  │────────>│   Postgres   │<────────│   Worker    │
│ (scheduler) │ Enqueue │  (Queue + DB)│  Claim  │  (worker)   │
└─────────────┘         └──────────────┘         └─────────────┘
                              ^
                              │
                         Workflow DSL
                       (process_order.flow)
```

## Files

- `tasks.py` - Task definitions decorated with `@task`
- `workflows/process_order.flow` - Workflow definition in Rhythm DSL
- `worker.py` - Worker entry point (executes tasks)
- `scheduler.py` - Scheduler entry point (enqueues work)

## Setup

1. **Start PostgreSQL:**
   ```bash
   createdb rhythm
   ```

2. **Set database URL (optional):**
   ```bash
   export RHYTHM_DATABASE_URL="postgresql://rhythm@localhost/rhythm"
   ```

## Running the Example

### Terminal 1: Start the Worker

```bash
cd python/examples/quickstart
python worker.py
```

The worker will:
- Initialize Rhythm and run migrations
- Register the workflow from `workflows/process_order.flow`
- Start polling for work on the `default` queue

### Terminal 2: Schedule Work

```bash
cd python/examples/quickstart
python scheduler.py
```

The scheduler will present a menu:
1. **Schedule a workflow** - Runs the full `processOrder` workflow
2. **Schedule standalone tasks** - Enqueues individual tasks
3. **Schedule both** - Runs workflow + standalone tasks
4. **Exit**

## What Happens

### Workflow Execution (`processOrder`)

When you schedule the workflow, Rhythm will:

1. **Create workflow execution** in the database
2. **Worker claims the workflow** from the queue
3. **Rust core executes the workflow**:
   - Parses the `.flow` file
   - Enqueues child tasks (`process_payment`, `update_inventory`, etc.)
   - Tracks workflow state
4. **Worker claims and executes each task**:
   - `process_payment` (sync function) - runs in thread pool
   - `update_inventory` (async function) - runs in event loop
   - `send_email` (async function)
   - `send_notification` (async function)
5. **Workflow completes** when all tasks finish

### Standalone Task Execution

Standalone tasks are executed directly without a workflow:
- Task is enqueued → Worker claims → Executes → Reports result

## Key Concepts Demonstrated

✅ **Task Decorators** - `@task` registers Python functions
✅ **Sync & Async Tasks** - Both are supported
✅ **Workflow DSL** - `.flow` files define multi-step workflows
✅ **Workflow Execution** - Rust core orchestrates workflow steps
✅ **Queue-Based Work** - Tasks flow through Postgres queue
✅ **Worker Loops** - Parallel claim+execute loops
✅ **Graceful Shutdown** - SIGINT/SIGTERM handling

## Monitoring

Watch the worker logs to see:
- Tasks being claimed
- Execution progress
- Task completion/failure
- Workflow state transitions

## Next Steps

- Add error handling to tasks
- Implement retry logic
- Add more complex workflows
- Create custom queues for prioritization
- Monitor execution status via the API
