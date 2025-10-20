# Project Pivot: DSL-Based Workflows

## Overview

The project is pivoting from Temporal-style workflows (code in host language with deterministic replay) to a custom DSL-based approach.

**Old Model (Temporal-style):**
```python
@currant.workflow
async def process_order(ctx, order_id):
    payment = await ctx.execute_activity(charge_card, order_id)
    await ctx.execute_activity(ship_order, order_id)
    # Deterministic replay on resume
```

**New Model (DSL-based):**
```
// processOrder.workflow (custom DSL file)
workflow processOrder(orderId: string): void {
  const payment = await chargeCard(orderId)
  await shipOrder(orderId)
  // State machine persists execution state, no replay
}
```

---

## Key Architectural Changes

### What Changes
1. **Workflows**: Now written in custom DSL (no more `@workflow` decorator in Python/Node)
2. **Workflow Storage**: DSL text + AST + execution state stored per workflow instance
3. **Execution Model**: Tree-walking interpreter with state capture (no replay)
4. **Versioning**: Each workflow instance stores its own DSL text (frozen at creation time)

### What Stays the Same
1. **Tasks**: Still implemented in Python/Node with `@task` decorator
2. **Task execution**: Same queue-based task execution model
3. **Worker architecture**: Workers still claim tasks from Postgres
4. **FFI layer**: Rust core still coordinates everything

---

## Core Design Principles

### 1. Extreme Simplicity
- **No closures**: Functions can't capture outer scope
- **No nested awaits**: Only top-level workflow can await
- **No recursion**: No workflow calling workflows
- **Flat execution**: Single stack frame, no call stack serialization

### 2. Clear Separation of Concerns
- **Workflows = Orchestration**: Just the "what" and "when"
- **Tasks = Business Logic**: The "how" lives here
- Complex data transformation, filtering, calculations ’ all in tasks
- Workflows are just glue code

### 3. Language-Agnostic Runtime
- Same DSL works with any host language (Python, Node, Go, etc.)
- DSL runtime lives in Rust core
- Tasks can be implemented in any language with FFI bindings

---

## Execution Model

### State Serialization (Simple)

Since no closures, no nested awaits, no recursion:

```json
{
  "workflow_instance_id": "order-123",
  "dsl_text": "workflow processOrder(...) { ... }",
  "ast": {/* cached parsed AST */},
  "execution_state": {
    "statement_index": 3,  // Which statement we're at
    "locals": {            // Local variables
      "orderId": "123",
      "payment": {"id": "pay_456", "amount": 99.99}
    },
    "awaiting_task_id": "task-789"  // If suspended
  }
}
```

**No call stack, no closures, no complex continuation capture needed.**

### Tree-Walking Interpreter

```rust
// Pseudocode
while stmt_index < ast.statements.len() {
    match ast.statements[stmt_index] {
        Statement::VarDecl { name, expr } => {
            locals[name] = eval_expr(expr, &locals);
            stmt_index += 1;
        }
        Statement::Await { task, args } => {
            // Suspend: save state and enqueue task
            save_state(stmt_index, &locals);
            enqueue_task(task, eval_args(args, &locals));
            return; // Worker will resume us when task completes
        }
        Statement::If { cond, then_branch, else_branch } => {
            if eval_expr(cond, &locals) {
                execute_branch(then_branch, &mut locals);
            } else {
                execute_branch(else_branch, &mut locals);
            }
            stmt_index += 1;
        }
        // ... loops, return, etc.
    }
}
```

---

## Minimal DSL Specification (Target State)

**Note:** Actual implementation will start MUCH simpler. Syntax is flexible and will evolve. Don't get attached to JS/TS syntax - let the right syntax emerge.

### Core Features (Eventually)
- Variables: `const`, `let`
- Await: `await taskName(args)`
- Conditionals: `if/else`
- Loops: `for`, `while`
- Basic operators: `+`, `-`, `*`, `/`, `<`, `>`, `==`, `!=`, `&&`, `||`, `!`
- Arrays/objects: Literals and property access
- Error handling: **No try/catch** - tasks return status objects

### Built-in Helpers (Maybe Eventually)
- Math: `Math.max()`, `Math.pow()`, etc.
- String methods: `toUpperCase()`, `split()`, etc.
- Array methods: `length`, `push()`, `join()`
- Object helpers: `Object.keys()`, etc.

### Explicitly Excluded
- Closures
- Higher-order functions (filter/map/reduce need lambdas)
- Nested async (await only in top-level workflow)
- Classes, imports, modules
- File I/O, network, timers (use tasks instead)
- Try/catch (tasks return success/error objects)

---

## Motivations for This Pivot

### Problems with Temporal-Style Workflows

1. **Determinism is Hard**
   - Need extensive tooling to prevent non-deterministic code
   - Different per language (Python linter, Node.js SDK, etc.)
   - Users constantly break determinism rules

2. **Replay is Complex**
   - Hard to understand for developers
   - Debugging is confusing (which execution am I looking at?)
   - Event history can grow unbounded

3. **Versioning is Manual**
   - Explicit version management required
   - Risk of breaking in-flight workflows on deploy
   - Need to maintain old code paths

4. **Language-Specific**
   - Each language needs full replay implementation
   - Code duplication across adapters
   - Inconsistent behavior between languages

### Benefits of DSL Approach

1. **No Determinism Issues**
   - DSL is inherently deterministic (limited feature set)
   - No need for linters or SDK safeguards
   - Impossible to write non-deterministic code

2. **No Replay Needed**
   - Execution state is explicit and serialized
   - Resume directly from where you left off
   - No event history to maintain

3. **Automatic Versioning**
   - Each instance stores its DSL text at creation time
   - Workflow is frozen - can never change
   - Deploy new workflow versions without breaking old instances

4. **Language-Agnostic**
   - Single DSL runtime in Rust core
   - Any language can implement tasks and call workflows
   - Consistent behavior everywhere

5. **Simpler Architecture**
   - No language-specific replay logic in adapters
   - Adapters just register tasks and queue workflows
   - All workflow execution in Rust core

6. **Better Observability**
   - Can parse DSL and visualize as DAG
   - Clear state transitions
   - No "replay history" confusion

---

## Trade-offs and Limitations

### What We Lose
- **Full language power**: Can't use arbitrary Python/Node code in workflows
- **Rich IDE support**: No LSP/autocomplete initially (can build later)
- **Native debugging**: Can't use Python/Node debugger on workflow code
- **Existing ecosystem**: Can't use language libraries in workflows

### What We Gain
- **Simplicity**: Smaller surface area, easier to reason about
- **Reliability**: Fewer ways to break workflows
- **Universality**: Same workflow works with any language
- **Predictability**: Limited DSL = predictable behavior

### Is This Acceptable?
**Yes**, because:
- Workflows are just orchestration (complex logic goes in tasks)
- Most workflow orchestration is simple (if/else, loops, await)
- AWS Step Functions is even more limited and widely used
- Users can still use full language power in tasks

---

## Implementation Strategy

### Phase 1: Proof of Concept (EXTREMELY BASIC)
**Goal:** Prove the core concept works

**Minimal DSL (intentionally crude, expect refactoring):**
- Sequential `await` statements only
- Simple variable assignment
- No loops, no conditionals, no operators
- Just prove: parse ’ execute ’ suspend ’ resume

**Example:**
```
workflow test(orderId):
  result1 = await task1(orderId)
  result2 = await task2(result1)
  return result2
```

**Focus:**
- Parse simple syntax to AST
- Store AST in database
- Walk AST, execute statements
- Serialize state on await (statement index + locals)
- Resume from serialized state

**NOT focusing on:**
- Pretty syntax
- Error messages
- Type checking
- Standard library
- Performance

### Phase 2: Basic Control Flow
Add:
- Conditionals: `if/else`
- Loops: `for`, `while`
- Operators: Comparison, logical, arithmetic

### Phase 3: Usability
Add:
- Better error messages
- Type annotations and checking
- Standard library (Math, String, Array helpers)
- Validation (detect infinite loops, unreachable code)

### Phase 4: Production Ready
Add:
- Performance optimizations
- Debugging tools
- DSL documentation
- Migration guide from old workflows

---

## Design Decisions

### Error Handling
**Decision:** No try/catch. Tasks return status objects.

```typescript
const result = await chargePayment(orderId)
if (!result.success) {
  await refundOrder(orderId)
  return
}
```

**Rationale:**
- Simpler to implement (no exception handling in interpreter)
- More explicit (error checking is visible)
- Matches "simple orchestration" philosophy
- Can add try/catch later if needed

### Parallel Execution (Promise.all)
**TBD:** Start without it, add later if needed.

### Helper Functions
**TBD:** May allow pure functions in DSL for readability.

### Standard Library
**TBD:** Start minimal, add as needed based on user feedback.

---

## Impact on Existing Project

### What Gets Removed
- Python `@workflow` decorator and replay logic
- Node.js `@workflow` decorator and replay logic
- Workflow replay mechanics in adapters
- Determinism validation tooling (never built, now not needed)

### What Gets Added
- DSL parser (in Rust core)
- DSL AST representation
- Tree-walking interpreter (in Rust core)
- Workflow state serialization/deserialization
- DSL file storage and retrieval

### What's Unchanged
- Task execution model
- Worker coordination
- Queue management
- Database schema (mostly - add workflow DSL storage)
- FFI architecture
- Python/Node task decorators

### Migration Impact
**Acceptable:** Project is pre-release, no users, breaking changes OK.

---

## Success Criteria

This pivot is successful if:

1. **Proof of concept works**: Can execute, suspend, and resume a simple workflow
2. **State serialization is reliable**: Workflows resume correctly after suspension
3. **Simpler than Temporal**: Easier to understand and use than deterministic replay
4. **Language-agnostic works**: Same DSL runs with Python and Node tasks
5. **Users prefer it**: Developers find it easier than writing workflow code

---

## Next Steps

1. **Design Phase 1 MVP**: Define absolute minimal DSL syntax
2. **Parser**: Choose parser approach (hand-written, nom, pest, etc.)
3. **AST**: Define Rust structs for AST representation
4. **Interpreter**: Build tree-walking executor
5. **State Persistence**: Serialize/deserialize execution state
6. **Integration**: Connect DSL runtime to existing task execution
7. **Testing**: Prove suspend/resume works correctly
