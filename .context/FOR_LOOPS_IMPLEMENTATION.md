# For Loops Implementation

## Summary

Successfully implemented full for loop support in the Rhythm workflow DSL, including:
- Basic for loop parsing and execution
- Await/suspension support within loop bodies
- Nested loops with proper scope management
- Mixed await and fire-and-forget tasks in loops

## Features Implemented

### 1. Parser Support

**Grammar** ([workflow.pest](../core/src/interpreter/workflow.pest)):
```pest
for_loop = { "for" ~ "(" ~ "let" ~ identifier ~ "in" ~ for_iterable ~ ")" ~ "{" ~ statement* ~ "}" }
for_iterable = { member_access | identifier | json_array }
```

**Note**: The `let` keyword is required when declaring the loop variable, consistent with the rest of the language's variable declaration syntax.

**Key Functions** ([parser.rs](../core/src/interpreter/parser.rs)):
- `parse_for_statement()` - Parses for loop syntax
- `parse_for_iterable()` - Handles inline arrays, variables, and member access
- Static scope resolution for loop variables at parse time

### 2. Executor Support

**Key Functions** ([executor.rs](../core/src/interpreter/executor.rs)):
- `resolve_iterable()` - Resolves iterable expressions at runtime
- For loop execution with state management
- Suspension/resumption logic for awaited tasks in loops

**Loop State Structure**:
```json
{
  "depth": 1,
  "scope_type": "for_loop",
  "variables": {
    "item": <current_item_value>
  },
  "metadata": {
    "loop_variable": "item",
    "collection": [1, 2, 3],
    "current_index": 0,
    "body_statement_index": 0
  }
}
```

### 3. Execution Model

**Without Await** (synchronous):
```flow
for (let item in [1, 2, 3]) {
  task("process", { value: item })  // Fire-and-forget
}
```
- All iterations execute in a single step
- No workflow suspension

**With Await** (asynchronous):
```flow
for (let order in inputs.orders) {
  await task("processOrder", { orderId: order.id })
}
```
- Each await suspends the workflow
- Loop state stored in `scope_stack`
- Resumes at next body statement after task completes
- Advances to next iteration after all body statements complete

### 4. Scope Management

**Depth Tracking**:
- Global scope: depth 0
- First for loop: depth 1
- Nested for loop: depth 2
- Variables annotated with depth at parse time: `{"var": "item", "depth": 1}`

**Resume Logic**:
- Check if innermost scope is `"for_loop"`
- If yes: Don't advance `statement_index`, continue loop execution
- If no: Advance `statement_index` to next top-level statement

### 5. Supported Iterable Types

1. **Inline Arrays**:
   ```flow
   for (let x in [1, 2, 3]) { ... }
   ```

2. **Variable References**:
   ```flow
   let items = await task("fetch", {})
   for (let item in items) { ... }
   ```

3. **Member Access**:
   ```flow
   for (let order in inputs.orders) { ... }
   ```

### 6. Nested Loops

Full support for nested loops with proper scope isolation:
```flow
for (let category in inputs.categories) {
  for (let product in category.products) {
    await task("process", {
      categoryId: category.id,
      productId: product.id
    })
  }
}
```

## Test Coverage

**Parser Tests** (8 tests):
- `test_simple_for_loop_inline_array`
- `test_for_loop_with_member_access_iterable`
- `test_for_loop_with_variable_iterable`
- `test_nested_for_loops`
- `test_for_loop_with_await`
- `test_for_loop_with_mixed_await`
- `test_for_loop_examples_workflow`

**Executor Tests** (6 tests):
- `test_resolve_iterable_inline_array`
- `test_resolve_iterable_variable_reference`
- `test_resolve_iterable_member_access`
- `test_resolve_iterable_complex_array`
- `test_lookup_scoped_variable_nested_scopes`
- `test_resolve_variables_nested_loop_scopes`

**Total**: 108 interpreter tests passing

## Example Workflows

See [for_loop_examples.flow](../python/examples/workflows/for_loop_examples.flow) for comprehensive examples including:
- Simple inline arrays
- Member access iterables
- Variable iterables
- Mixed await/fire-and-forget
- Nested loops
- Complex data transformations

## Technical Notes

### Performance
- Fire-and-forget loops execute synchronously (all iterations in one step)
- Awaited tasks cause workflow suspension per iteration
- Loop state stored in JSONB `locals` field (minimal overhead)

### Limitations
- Only task statements supported in loop bodies currently
- No early loop exit (break/continue) yet
- Iterable must be an array (no generators/iterators)

### Future Enhancements
- Support for `break` and `continue` statements
- Support for if/else statements in loop bodies
- Support for nested scoped statements (let, etc.) in loop bodies
- Range syntax: `for (i in 0..10)`
- Parallel loop execution option
