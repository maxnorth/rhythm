# Scoped Variables & Loop Implementation Design

## Design Date
November 1, 2025

## Goals

1. **Implement lexically scoped variables** - Support block-scoped variables (if, for, try/catch, etc.)
2. **Enable for loops** - The primary motivating use case
3. **Maintain flat state** - Everything must serialize to a single JSONB column
4. **Optimize for performance** - Static scope resolution at parse time
5. **Future-proof** - Design that extends to try/catch, while loops, arbitrary blocks

## Core Principles

### 1. Flat State Storage
All execution state must fit in the existing `locals` JSONB column. No new database columns.

**Current schema:**
```sql
CREATE TABLE workflow_execution_context (
    execution_id TEXT PRIMARY KEY,
    workflow_definition_id INTEGER NOT NULL,
    statement_index INTEGER NOT NULL DEFAULT 0,
    locals JSONB NOT NULL DEFAULT '{}',      -- ← Everything goes here
    awaiting_task_id TEXT,
    created_at TIMESTAMP,
    updated_at TIMESTAMP
);
```

### 2. No Backward Compatibility Required
Old workflows with the previous `locals` format can break. We're free to make breaking changes.

### 3. Scope Lifecycle
- **Enter block** → Create new scope
- **Execute block** → Variables exist in that scope
- **Exit block** → Pop scope (all variables destroyed)
- **Loop iterations** → Same scope, update metadata

## Scope Structure

### State Format

```json
{
  "statement_index": 5,
  "awaiting_task_id": null,
  "locals": {
    "scope_stack": [
      {
        "depth": 0,
        "scope_type": "global",
        "variables": {
          "orderId": "123",
          "userId": "user456"
        }
      },
      {
        "depth": 1,
        "scope_type": "if",
        "variables": {
          "status": "ok"
        }
      },
      {
        "depth": 2,
        "scope_type": "for_loop",
        "variables": {
          "item": "current_value"
        },
        "metadata": {
          "collection": ["a", "b", "c"],
          "current_index": 1
        }
      }
    ]
  }
}
```

### Scope Fields

**Required for all scopes:**
- `depth` (number) - Nesting level, 0 = global
- `scope_type` (string) - "global", "if", "for_loop", "try", "catch", etc.
- `variables` (object) - User-defined variables in this scope

**Optional:**
- `metadata` (object) - Scope-specific internal state (loop iteration, error context, etc.)

### Why `metadata` Instead of Special Variables?

**Decision: Use separate `metadata` field**

Alternatives considered:
```json
// Option A: Special variables (rejected)
{
  "variables": {
    "item": "value",
    "$__collection": [...],  // Pollutes namespace
    "$__index": 1
  }
}

// Option B: Metadata field (chosen)
{
  "variables": {
    "item": "value"
  },
  "metadata": {
    "collection": [...],
    "current_index": 1
  }
}
```

**Rationale for metadata:**
1. **Clear separation** - User variables vs system state
2. **Extensible** - Easy to add new scope types (try/catch error info, while loop conditions)
3. **Type safety** - Can validate structure per scope_type
4. **Debugging** - Immediately clear what's user data vs machinery
5. **No collision risk** - Users can't accidentally override internal state

## Variable Resolution

### Static Scope Resolution (Parse Time)

**Key Optimization:** Determine variable scope depth during parsing, not at runtime.

**Instead of runtime walking:**
```rust
// ❌ Old approach - O(depth)
for scope in scope_stack.iter().rev() {
    if let Some(val) = scope.variables.get(name) {
        return val;
    }
}
```

**Use static indexing:**
```rust
// ✅ New approach - O(1)
let scope = &scope_stack[variable_ref.scope_depth];
let value = scope.variables.get(variable_ref.name);
```

### Parser Symbol Table

Parser maintains symbol table while parsing:

```rust
struct ParserState {
    scope_depth: usize,
    // Stack of scopes, each maps variable name → depth where it's defined
    symbol_table: Vec<HashMap<String, usize>>,
}
```

**When parsing variable reference:**
1. Look up variable in symbol table (walk from current depth to 0)
2. Find the depth where it's defined
3. Annotate the reference with that depth

**Example:**
```flow
let orderId = "123"           // Define at depth 0
for (item in items) {         // Enter depth 1
  task("log", {
    orderId: orderId,         // Parser annotates: scope_depth: 0
    item: item                // Parser annotates: scope_depth: 1
  })
}
```

**Parser output:**
```json
{
  "type": "task",
  "task": "log",
  "inputs": {
    "orderId": {
      "type": "variable_ref",
      "name": "orderId",
      "scope_depth": 0
    },
    "item": {
      "type": "variable_ref",
      "name": "item",
      "scope_depth": 1
    }
  }
}
```

### Variable Shadowing

Variables in inner scopes shadow outer scopes (standard lexical scoping):

```flow
let x = 1                    // x at depth 0
for (x in [2, 3]) {         // x at depth 1 (shadows depth 0)
  task("log", { x: x })     // References depth 1
}
task("log", { x: x })       // References depth 0 again
```

Parser resolves by walking symbol table from innermost to outermost scope.

### Undefined Variables

**Behavior:** Variables referenced before definition are caught at parse time.

```flow
task("log", { x: x })        // Parse error: undefined variable 'x'
let x = 1
```

**Exception:** Variables defined in outer scopes are valid:
```flow
let x = 1
if (condition) {
  task("log", { x: x })      // ✅ Valid - x defined at outer scope
}
```

**Future:** Compile-time validation and IDE linting will catch these errors.

## Scope Management in Executor

### Scope Stack Operations

**Push scope (enter block):**
```rust
scope_stack.push(Scope {
    depth: current_depth + 1,
    scope_type: "for_loop",
    variables: HashMap::new(),
    metadata: Some(json!({
        "collection": [...],
        "current_index": 0
    })),
});
```

**Pop scope (exit block):**
```rust
// Simply remove all scopes at or above target depth
scope_stack.retain(|s| s.depth < exit_depth);
```

**Variable assignment:**
```rust
// Use static depth from parse time
scope_stack[depth].variables.insert(name, value);
```

**Variable lookup:**
```rust
// Direct index - O(1)
let value = scope_stack[depth].variables.get(name)?;
```

### When to Write to Database

**Only snapshot state when:**
1. ✅ Workflow suspends (hits an await)
2. ✅ Workflow completes
3. ✅ Task completes and workflow resumes

**Never snapshot:**
- ❌ Between statements in a sequence
- ❌ On each loop iteration (unless iteration hits await)
- ❌ When entering/exiting scopes (unless at suspension point)

This minimizes database writes and maximizes performance.

### Scope Lifecycle Example

```flow
workflow(ctx, inputs) {                    // Create scope 0
  let orderId = await task("create", {})   // Suspend: snapshot with scope 0

  if (orderId != null) {                   // Create scope 1
    let status = await task("check", {})   // Suspend: snapshot with scopes 0, 1

    for (item in items) {                  // Create scope 2
      await task("process", { item })      // Suspend: snapshot with scopes 0, 1, 2
    }
    // Exit for loop: Pop scope 2
  }
  // Exit if block: Pop scope 1
}
// Exit workflow: Pop scope 0 (if needed)
```

## For Loop Implementation

### Syntax

```flow
for (variable_name in iterable) {
  // body statements
}
```

**Iterables:**
- Variable reference: `for (item in items)`
- Member access: `for (item in inputs.items)`
- Inline array: `for (item in [1, 2, 3])`

### Parsed Structure

```json
{
  "type": "for",
  "creates_scope": true,
  "loop_variable": "item",
  "iterable": {
    "type": "variable",  // or "member_access", "array"
    "value": "$items"
  },
  "body_statements": [
    {
      "type": "task",
      "task": "process",
      "inputs": {
        "item": {
          "type": "variable_ref",
          "name": "item",
          "scope_depth": 1  // Assumes for loop at depth 0
        }
      }
    }
  ]
}
```

### Execution Flow

**1. Enter Loop (First Time)**
```
1. Evaluate iterable → get collection array
2. Create scope at depth N+1:
   - scope_type: "for_loop"
   - variables: {} (empty initially)
   - metadata: { collection: [...], current_index: 0 }
3. Set loop variable: scope.variables[loop_var] = collection[0]
4. Execute body statements
```

**2. Suspend During Loop**
```
- Hit await in loop body
- Snapshot entire scope_stack (including loop metadata)
- Write to DB
- Return Suspended
```

**3. Resume Loop**
```
1. Load scope_stack from DB
2. Find loop scope (scope_type: "for_loop")
3. Read current_index from metadata
4. Continue executing body from suspension point
```

**4. Next Iteration**
```
1. Body completes (or fire-and-forget task)
2. Increment metadata.current_index
3. Check if current_index >= collection.length
   - If yes: Pop scope, move to next statement
   - If no: Update loop variable, execute body again
```

**5. Exit Loop**
```
1. current_index >= collection.length
2. Pop scope (removes loop variable and metadata)
3. Continue to next statement
```

### Loop State Storage

Stored in scope's `metadata` field:

```json
{
  "depth": 1,
  "scope_type": "for_loop",
  "variables": {
    "item": "current_value"
  },
  "metadata": {
    "collection": ["a", "b", "c"],   // Snapshot of array at loop entry
    "current_index": 1                // Which iteration we're on
  }
}
```

**Why snapshot collection?**
- Predictable behavior - collection doesn't change mid-loop
- Matches JavaScript `for...of` semantics
- Simpler implementation
- Reproducible execution

### Nested Loops

Each loop creates its own scope:

```flow
for (order in orders) {              // Scope depth 1
  for (item in order.items) {        // Scope depth 2
    await task("process", {
      order: order,                  // From depth 1
      item: item                     // From depth 2
    })
  }
}
```

**State with nested loops:**
```json
{
  "scope_stack": [
    {"depth": 0, "variables": {"orders": [...]}},
    {
      "depth": 1,
      "scope_type": "for_loop",
      "variables": {"order": {...}},
      "metadata": {"collection": [...], "current_index": 0}
    },
    {
      "depth": 2,
      "scope_type": "for_loop",
      "variables": {"item": {...}},
      "metadata": {"collection": [...], "current_index": 3}
    }
  ]
}
```

### Loop + If Combination

```flow
for (item in items) {                // Scope depth 1
  if (item.priority == "high") {     // Scope depth 2
    let urgent = true                // Variable at depth 2
    await task("expedite", { item })
  }
}
```

Scope is popped when if block exits, but loop continues.

## Statements That Create Scopes

### Current Implementation

**For loops:** ✅ Create scope
**If/else:** Not yet - will add in future
**Try/catch:** Not yet - future work

### Future Scope Types

**If blocks (future):**
```flow
if (condition) {                     // Creates scope
  let localVar = await task(...)     // Variable at if scope
}
// localVar no longer accessible
```

**Try/catch blocks (future):**
```json
{
  "depth": 1,
  "scope_type": "try",
  "variables": {...}
},
{
  "depth": 1,
  "scope_type": "catch",
  "variables": {"error": {...}},
  "metadata": {
    "error_type": "TaskFailure",
    "error_message": "..."
  }
}
```

**While loops (future):**
```json
{
  "depth": 1,
  "scope_type": "while_loop",
  "variables": {...},
  "metadata": {
    "iteration_count": 5,
    "max_iterations": 100  // Safety limit
  }
}
```

## Parser Changes Required

### 1. Add Symbol Table

```rust
struct ParserContext {
    scope_depth: usize,
    symbol_table: Vec<HashMap<String, usize>>,
}

impl ParserContext {
    fn enter_scope(&mut self) {
        self.scope_depth += 1;
        self.symbol_table.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        self.symbol_table.pop();
        self.scope_depth -= 1;
    }

    fn declare_variable(&mut self, name: String) {
        self.symbol_table[self.scope_depth].insert(name, self.scope_depth);
    }

    fn lookup_variable(&self, name: &str) -> Option<usize> {
        // Walk from current scope to global
        for depth in (0..=self.scope_depth).rev() {
            if self.symbol_table[depth].contains_key(name) {
                return Some(depth);
            }
        }
        None
    }
}
```

### 2. Annotate Variable References

Change variable references from:
```json
{"type": "identifier", "value": "$varName"}
```

To:
```json
{
  "type": "variable_ref",
  "name": "varName",
  "scope_depth": 1
}
```

### 3. Track Scope-Creating Statements

Add `creates_scope` flag:
```json
{
  "type": "for",
  "creates_scope": true,
  "loop_variable": "item",
  "iterable": {...},
  "body_statements": [...]
}
```

### 4. Parse-Time Validation

Detect undefined variables:
```flow
task("log", { x: undefinedVar })  // Parse error!
```

Return helpful error:
```
Parse error on line 3: Undefined variable 'undefinedVar'
```

## Executor Changes Required

### 1. Load Scope Stack

```rust
let context: Option<(i32, i32, JsonValue, Option<String>)> = sqlx::query_as(
    "SELECT workflow_definition_id, statement_index, locals, awaiting_task_id
     FROM workflow_execution_context WHERE execution_id = $1"
)
.bind(execution_id)
.fetch_optional(pool)
.await?;

let (workflow_def_id, statement_index, locals_json, awaiting_task_id) = context?;

// Parse scope stack from locals
let scope_stack: Vec<Scope> = serde_json::from_value(
    locals_json["scope_stack"].clone()
)?;
```

### 2. Update Variable Resolution

```rust
fn resolve_variables(value: &JsonValue, scope_stack: &[Scope]) -> JsonValue {
    match value {
        JsonValue::Object(obj) if obj.contains_key("type")
            && obj["type"] == "variable_ref" => {
            // Direct lookup - O(1)
            let name = obj["name"].as_str()?;
            let depth = obj["scope_depth"].as_u64()? as usize;

            scope_stack.get(depth)
                .and_then(|s| s.variables.get(name))
                .cloned()
                .unwrap_or(value.clone())
        }
        JsonValue::Array(arr) => {
            JsonValue::Array(
                arr.iter()
                    .map(|v| resolve_variables(v, scope_stack))
                    .collect()
            )
        }
        JsonValue::Object(obj) => {
            JsonValue::Object(
                obj.iter()
                    .map(|(k, v)| (k.clone(), resolve_variables(v, scope_stack)))
                    .collect()
            )
        }
        _ => value.clone()
    }
}
```

### 3. Scope Management Helpers

```rust
fn enter_scope(
    scope_stack: &mut Vec<Scope>,
    scope_type: &str,
    metadata: Option<JsonValue>
) -> usize {
    let depth = scope_stack.len();
    scope_stack.push(Scope {
        depth,
        scope_type: scope_type.to_string(),
        variables: HashMap::new(),
        metadata,
    });
    depth
}

fn exit_scope(scope_stack: &mut Vec<Scope>, target_depth: usize) {
    scope_stack.retain(|s| s.depth < target_depth);
}
```

### 4. For Loop Execution

```rust
"for" => {
    let loop_var = statement["loop_variable"].as_str()?;
    let current_scope = scope_stack.last();

    // Check if we're resuming or starting
    let (collection, current_index) = if let Some(scope) = current_scope {
        if scope.scope_type == "for_loop" {
            // Resuming - load from metadata
            let coll = scope.metadata["collection"].as_array()?.clone();
            let idx = scope.metadata["current_index"].as_u64()? as usize;
            (coll, idx)
        } else {
            // Starting new loop
            let coll = evaluate_iterable(&statement["iterable"], scope_stack)?;
            enter_scope(scope_stack, "for_loop", Some(json!({
                "collection": coll.clone(),
                "current_index": 0
            })));
            (coll, 0)
        }
    } else {
        unreachable!("Should always have at least global scope");
    };

    // Check if loop is done
    if current_index >= collection.len() {
        exit_scope(scope_stack, scope_stack.len() - 1);
        // Move to next statement
        return Ok(StepResult::Continue);
    }

    // Set loop variable
    let loop_scope = scope_stack.last_mut()?;
    loop_scope.variables.insert(loop_var.to_string(), collection[current_index].clone());

    // Execute body...
}
```

## Testing Strategy

### Unit Tests

1. **Parser scope tracking**
   - Variable declared and referenced in same scope
   - Variable referenced from outer scope
   - Variable shadowing
   - Undefined variable error

2. **Executor scope management**
   - Push/pop scopes correctly
   - Variable resolution with scope depth
   - Scope metadata preserved across suspend/resume

3. **For loop basic**
   - Simple loop over inline array
   - Loop over variable
   - Loop over member access (inputs.items)

4. **For loop with await**
   - Suspend in loop body
   - Resume and continue iteration
   - Complete loop and move to next statement

5. **Nested loops**
   - Two levels of nesting
   - Access outer loop variable from inner loop
   - Suspend in nested loop

6. **Loop + If**
   - If statement inside loop
   - Variable defined in if, inaccessible after if
   - Loop variable accessible in if

### Integration Tests

1. Real workflow with for loop
2. Order processing example (iterate over items)
3. Batch processing workflow
4. Error handling in loop body

## Performance Considerations

### Parse Time
- Symbol table lookup: O(depth) per variable reference
- Typical depth: < 5 levels
- Negligible impact on parsing

### Runtime
- Variable resolution: O(1) direct indexing (was O(depth) walking)
- Scope push/pop: O(1) operations
- Database writes: Only on suspend/complete (unchanged)

### Memory
- Scope stack size: ~100 bytes per scope
- Typical max depth: 3-5 scopes
- Loop collections: Snapshot at entry (could be large for big arrays)
  - Future optimization: Store reference instead of copy for large collections

## Future Enhancements

### Short Term
1. Add scope support to if/else blocks
2. Implement while loops
3. Add break/continue statements

### Medium Term
1. Try/catch blocks with error scoping
2. Compile-time variable validation
3. IDE linting for undefined variables
4. Better error messages with scope context

### Long Term
1. Optimize large collection handling (references instead of snapshots)
2. Parallel for loops (fan-out pattern)
3. Generator/iterator patterns
4. Scope-based permissions (isolate untrusted code)

## Migration Notes

### Breaking Changes
- ✅ Old `locals` format will break (acceptable)
- ✅ No automatic migration (workflows must be re-registered)

### What Breaks
Old workflows with:
```json
{"locals": {"orderId": "123"}}
```

Now require:
```json
{
  "locals": {
    "scope_stack": [
      {"depth": 0, "scope_type": "global", "variables": {"orderId": "123"}}
    ]
  }
}
```

### Detection
Executor checks for `scope_stack` field:
- Present → New format
- Missing → Error with clear message to re-register workflow

## Open Questions

None - design is complete and ready for implementation.

## Implementation Checklist

- [ ] Update parser to track symbol table
- [ ] Annotate variable references with scope_depth
- [ ] Add creates_scope flag to scope-creating statements
- [ ] Update resolve_variables to use scope_depth
- [ ] Add scope stack to executor state
- [ ] Implement enter_scope/exit_scope helpers
- [ ] Implement for loop execution with scope management
- [ ] Add parse-time undefined variable detection
- [ ] Write comprehensive tests
- [ ] Update documentation

---

**Approved By:** [Pending implementation]
**Implementation Date:** November 1, 2025
**Status:** Design Complete - Ready for Implementation
