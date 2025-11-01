# ⚠️ CRITICAL: NO BACKWARD COMPATIBILITY REQUIRED ⚠️

## Project Stage: Pre-Release Development

This project is in **active development** and has:
- **NO users yet**
- **NO public release**
- **NO production deployments**
- **NO legacy workflows to support**

## Development Philosophy

### ✅ DO THIS:
- Make breaking changes freely
- Simplify the codebase aggressively
- Choose the cleanest design without compromise
- Delete old code paths completely when replacing them
- Use a single, straightforward implementation
- **Break tests when the design changes** - tests reflect old designs
- Rewrite tests to match new semantics rather than preserving old behavior

### ❌ NEVER DO THIS:
- Add "backward compatibility" code paths
- Support "old format" alongside "new format"
- Keep fallback logic for "legacy workflows"
- Add migration code for non-existent users
- Complicate implementations to support phantom past versions
- Preserve old test behavior at the expense of clean design

## Why This Matters

**Every line of backward compatibility code is technical debt that:**
1. Increases cognitive load for future development
2. Makes the codebase harder to understand
3. Adds unnecessary branches and complexity
4. Slows down development velocity
5. Makes testing more complicated

**We have NO legacy to support.** Any "old format" mentioned in code comments or implementations is just a prototype from last week that should be completely replaced, not accommodated.

## Examples of What NOT to Do

### ❌ BAD - Unnecessary backward compatibility:
```rust
fn resolve_variables(value: &JsonValue, locals: &JsonValue) -> JsonValue {
    match value {
        JsonValue::String(s) => {
            // Check for old-style variable reference (starts with $)
            if let Some(var_name) = s.strip_prefix('$') {
                // Linear search for backward compatibility
                return lookup_variable_linear(var_name, locals);
            }
            // ... more code
        }
        JsonValue::Object(obj) => {
            // Check if this is new format with scope depth
            if let (Some(var), Some(depth)) = (obj.get("var"), obj.get("depth")) {
                return lookup_scoped_variable(var, depth, locals);
            }
            // ... fallback logic
        }
    }
}
```

### ✅ GOOD - Single clean implementation:
```rust
fn resolve_variables(value: &JsonValue, locals: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(obj) => {
            // Variable references are always annotated with scope depth
            if let (Some(var), Some(depth)) = (obj.get("var"), obj.get("depth")) {
                return lookup_scoped_variable(var, depth, locals);
            }
            // Not a variable - resolve recursively
            resolve_object_values(obj, locals)
        }
        // ... other cases
    }
}
```

## Tests Are Not Sacred

**CRITICAL INSIGHT: Tests reflect the design at the time they were written.**

When you make a breaking change to improve the design:
- Tests WILL fail - this is EXPECTED and GOOD
- Failing tests show you what changed
- Fix tests to match the new design
- Don't preserve old behavior to keep tests passing

### Example:
If you change variable syntax from `$varname` to `{"var": "name", "depth": 0}`:
- ✅ Update all tests to use the new format
- ❌ Don't support both formats to avoid updating tests

**Tests are tools to verify current behavior, not historical artifacts to preserve.**

## When You See These Phrases - STOP:

- "for backward compatibility"
- "fallback to old format"
- "support legacy workflows"
- "old-style reference"
- "if new format exists, otherwise..."
- "migrate existing workflows"
- "keep tests passing without changing them"

**These are red flags.** There is nothing to be backward compatible WITH.

## The Rule

**If you're replacing an implementation, DELETE the old one completely. Don't support both.**

**If tests break, FIX THE TESTS to match the new design.**

This is not a "maybe" or a "nice to have" - this is a hard requirement for keeping the codebase maintainable during rapid development.

---

**Last Updated**: 2025-11-01
**Reminder Frequency**: Read this EVERY time you consider adding "backward compatibility"
