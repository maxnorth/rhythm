# Future Refactoring Notes

This document tracks design improvements and refactoring work to be done later.

## Parser Improvements

**Priority:** Medium
**Status:** To Do

### Issue: Parse errors should be semantic validation errors

Currently the parser rejects workflows with helpful error messages at parse time, but these should really be semantic validation errors:

1. **Function signature validation**: `workflow test_name(inputs)` is rejected as a parse error, but the syntax is valid - it should parse and then fail semantic validation with "workflow names are not supported in signature" or similar
2. **Assignment of await expressions**: `result = await Task.run()` is rejected as a parse error ("expected statement"), but this is valid syntax that should parse and potentially be supported

**Desired behavior:**
- Parser should accept any syntactically valid JavaScript-like code
- Semantic validator should check:
  - Workflow signature is `workflow(ctx, inputs)`
  - Unsupported features are used (if `result = await` is not supported yet)
  - Type checking and other semantic rules

This separation makes error messages clearer and makes it easier to add language features incrementally.

---

## Separate Execution Queue Table

**Priority:** High (performance-critical)
**Status:** To Do

### Current State

Workers query the `executions` table directly to find pending work:

```sql
SELECT * FROM executions
WHERE status = 'pending'
ORDER BY priority, created_at
LIMIT 1
FOR UPDATE SKIP LOCKED
```

As completed/failed tasks accumulate in the `executions` table, performance degrades:
- Index bloat on `idx_executions_status_priority`
- Workers scan past many completed tasks to find pending ones
- Table bloat affects cache hit rates
- Vacuum overhead increases

### Desired State

Create a separate, fast queue table that only contains pending/running tasks:

```sql
CREATE TABLE execution_queue (
    execution_id TEXT PRIMARY KEY REFERENCES executions(id) ON DELETE CASCADE,
    queue TEXT NOT NULL,
    priority INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running'))
);

CREATE INDEX idx_queue_claim ON execution_queue(queue, priority, created_at)
WHERE status = 'pending';
```

Workers query this small table instead:

```sql
SELECT execution_id FROM execution_queue
WHERE queue = ANY($1) AND status = 'pending'
ORDER BY priority DESC, created_at ASC
LIMIT 1
FOR UPDATE SKIP LOCKED
```

When a task completes/fails, remove it from the queue table. The `executions` table becomes purely historical/audit trail.

### Benefits

- **Fast claims**: Queue table stays small (only active tasks)
- **No index bloat**: Completed tasks don't pollute the index
- **Better cache hit rate**: Queue table fits in memory
- **Proven pattern**: Used by Sidekiq, BullMQ, Temporal, and other production job queues
- **Easy archival**: Old executions can be moved to archive table without affecting queue

### Migration Plan

1. Create `execution_queue` table
2. Backfill with current pending/running executions
3. Update `create_execution()` to insert into both tables
4. Update `claim_execution()` to query from queue table
5. Update `complete_execution()` and `fail_execution()` to remove from queue
6. Add periodic archival job to move old executions to archive table

### Blockers

None - just deferred to avoid blocking workflow continuation work.

---

## Execution Arguments Schema Simplification

**Priority:** Medium
**Status:** To Do

### Current State

The `executions` table has separate `args` and `kwargs` columns:

```sql
CREATE TABLE executions (
    ...
    args JSONB,    -- Array: [1, 2, 3]
    kwargs JSONB,  -- Object: {"key": "value"}
    ...
);
```

### Desired State

Simplify to a single `arguments` column that is always a JSON object:

```sql
CREATE TABLE executions (
    ...
    arguments JSONB,  -- Always object: {"arg1": 1, "key": "value"}
    ...
);
```

### Benefits

- Cleaner API - single argument pattern
- More consistent interface across language bridges (Python, Node, Rust)
- Simpler query building (no need to handle both args and kwargs separately)
- Easier to document and reason about

### Migration Plan

1. Add `arguments` column to `executions` table
2. Backfill existing rows: merge `args` and `kwargs` into `arguments`
3. Update all code that writes to `executions` table
4. Update all code that reads from `executions` table
5. Deprecate `args` and `kwargs` columns
6. Eventually drop old columns in future migration

### Blockers

None - just deferred to avoid blocking workflow continuation work.
