# Scoped Variables & For Loops - Quick Reference

## TL;DR

Implementing lexically scoped variables with static scope resolution for O(1) variable lookup. Primary use case: for loops.

## Key Decisions

1. **Storage:** Everything in `locals.scope_stack` - no new DB columns
2. **Resolution:** Static scope depth determined at parse time - O(1) runtime lookup
3. **Metadata:** Use separate `metadata` field for loop state (not special variables)
4. **Breaking:** Old workflows will break, no migration needed
5. **Depth:** Scopes indexed by nesting depth (easy to pop on exit)

## State Format

```json
{
  "locals": {
    "scope_stack": [
      {
        "depth": 0,
        "scope_type": "global",
        "variables": {"orderId": "123"}
      },
      {
        "depth": 1,
        "scope_type": "for_loop",
        "variables": {"item": "current"},
        "metadata": {
          "collection": ["a", "b", "c"],
          "current_index": 1
        }
      }
    ]
  }
}
```

## Parser Changes

**Add symbol table:**
```rust
struct ParserContext {
    scope_depth: usize,
    symbol_table: Vec<HashMap<String, usize>>,
}
```

**Annotate variables with scope:**
```json
{
  "type": "variable_ref",
  "name": "item",
  "scope_depth": 1
}
```

## Executor Changes

**Variable resolution:**
```rust
// O(1) - direct indexing instead of walking
let value = scope_stack[var_ref.scope_depth].variables.get(var_ref.name);
```

**Scope operations:**
```rust
enter_scope(scope_stack, "for_loop", metadata);  // Push
exit_scope(scope_stack, target_depth);           // Pop
```

## For Loop Syntax

```flow
for (item in items) {
  await task("process", { item: item })
}
```

**Iterables:**
- Variables: `for (x in items)`
- Member access: `for (x in inputs.items)`
- Inline arrays: `for (x in [1, 2, 3])`

## Performance

- Parse: O(depth) per variable, depth typically < 5
- Runtime: O(1) variable lookup (was O(depth))
- DB writes: Only on suspend/complete (unchanged)

## See Full Design

[SCOPED_VARIABLES_DESIGN.md](./SCOPED_VARIABLES_DESIGN.md) - Complete specification with rationale and examples
