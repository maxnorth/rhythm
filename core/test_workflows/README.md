# Workflow DSL Test Suite

This directory contains test workflow files demonstrating various syntax patterns and features of the Rhythm workflow DSL.

## Test Workflow Files

### 1. **basic_sequential.flow**
Tests basic sequential execution with `await` keyword.
- Expected behavior: Tasks execute one after another, each waiting for completion
- Validates: `await` keyword, sequential execution order

### 2. **fire_and_forget.flow**
Tests fire-and-forget task execution (no `await`).
- Expected behavior: All tasks are queued immediately without blocking
- Validates: Non-blocking task execution, workflow completes without waiting

### 3. **mixed_await.flow**
Tests combination of awaited and fire-and-forget tasks.
- Expected behavior: `await` tasks block, non-`await` tasks don't
- Validates: Mixed execution patterns, proper async handling

### 4. **variables_simple.flow**
Tests basic variable assignment and usage.
- Expected behavior: Task result is captured in variable and passed to next task
- Validates: `let` statement, variable capture, variable references

### 5. **variables_multiple.flow**
Tests multiple variables with data flow between tasks.
- Expected behavior: Multiple variables tracked independently, proper data passing
- Validates: Multiple variable assignments, variable composition

### 6. **variables_fire_and_forget.flow**
Tests variable assignment with non-awaited tasks.
- Expected behavior: Variables assigned but values may not be available immediately
- Validates: Assignment from fire-and-forget tasks

### 7. **json_types.flow**
Tests all JSON data types in task inputs.
- Expected behavior: Parser correctly handles all JSON primitives and structures
- Validates: Strings, numbers, booleans, null, arrays, nested objects

### 8. **variables_in_complex_json.flow**
Tests variable references in nested JSON structures.
- Expected behavior: Variables resolved in arrays and nested objects
- Validates: Deep variable resolution, mixed data structures

## Test Coverage

### Parsing Tests (16 tests in `parser.rs`)
- ✅ Simple workflows with task calls
- ✅ `await` keyword parsing
- ✅ Variable assignment (`let varname = ...`)
- ✅ Variable references (bare identifiers)
- ✅ All JSON types (string, number, boolean, null, array, object)
- ✅ Nested JSON structures
- ✅ Comments and whitespace
- ✅ Single and double quotes
- ✅ Variable naming conventions
- ✅ Error handling for invalid syntax

### Variable Resolution Tests (11 tests in `executor.rs`)
- ✅ Simple string variable resolution
- ✅ Variable resolution in objects
- ✅ Variable resolution in arrays
- ✅ Deeply nested variable resolution
- ✅ Complex type variables (objects, arrays, primitives)
- ✅ Missing variable handling (keeps `$varname`)
- ✅ Mixed found/missing variables
- ✅ Empty locals handling
- ✅ Non-variable dollar signs preserved
- ✅ All primitive types (numbers, booleans, null)
- ✅ Variables in array of objects

### Workflow Integration Tests (15 tests in `workflows.rs`)
- ✅ Basic sequential workflows
- ✅ Fire-and-forget workflows
- ✅ Mixed await patterns
- ✅ Simple variable workflows
- ✅ Multiple variable workflows
- ✅ Fire-and-forget with variables
- ✅ All JSON types
- ✅ Variables in complex JSON
- ✅ Comments and whitespace handling
- ✅ Single quote strings
- ✅ Variable naming conventions
- ✅ Workflow registration
- ✅ Parse error handling

## Syntax Reference

### Task Execution
```
# Awaited task (blocks until complete)
await task("task_name", { "key": "value" })

# Fire-and-forget task (doesn't block)
task("task_name", { "key": "value" })
```

### Variable Assignment
```
# Capture task result in variable
let result = await task("create_user", { "name": "Alice" })

# Use variable in subsequent task (bare identifier)
await task("send_email", { "user_id": result })
```

### JSON Data Types
```
await task("example", {
    "string": "hello",
    "number": 42,
    "float": 3.14,
    "bool": true,
    "null": null,
    "array": [1, 2, 3],
    "object": { "nested": "value" },
    "variable_ref": my_variable
})
```

### Comments
```
# This is a comment
await task("task1", {})  # Comments are ignored
```

## Running Tests

Run all interpreter tests:
```bash
cargo test --lib interpreter
```

Run workflow integration tests:
```bash
cargo test --lib workflows::tests
```

Run specific test:
```bash
cargo test --lib test_workflow_variables_simple
```

## Test Results Summary

**Total Tests: 42**
- Parser Tests: 16/16 passing ✅
- Executor Tests: 11/11 passing ✅
- Workflow Tests: 15/15 passing ✅

All tests validate both parsing (syntax correctness) and execution behavior (runtime semantics).
