# JavaScript Semantic Deviations

This document tracks intentional semantic differences between the Rhythm workflow language and JavaScript.

## Type System

### No `undefined` - Only `null`

**Deviation:** Rhythm does not have an `undefined` value. All absence-of-value semantics use `null`.

**JavaScript behavior:**
```javascript
let x;              // undefined
let obj = {};
obj.missing;        // undefined
```

**Rhythm behavior:**
```javascript
let x = null;       // Must explicitly initialize (or defaults to null)
let obj = {};
obj.missing;        // Runtime error (see below)
```

**Rationale:** Simplifies the type system and reduces confusion between "no value" (`undefined`) and "explicitly no value" (`null`). Workflows benefit from explicit value handling.

---

## Property Access

### Non-existent Property Access is an Error

**Deviation:** Accessing a property that doesn't exist on an object throws a runtime error.

**JavaScript behavior:**
```javascript
let obj = { a: 1 };
obj.missing;        // undefined (no error)
```

**Rhythm behavior:**
```javascript
let obj = { a: 1 };
obj.missing;        // Runtime error: "Property 'missing' does not exist"
```

**Rationale:** Makes workflow errors explicit and easier to debug. Typos in property names are caught immediately rather than propagating `undefined` through the system. Workflows should fail fast on structural errors.

**Workaround:** Use the optional chaining operator (`?.`) when a property might not exist:
```javascript
obj?.missing;       // null if property doesn't exist (no error)
```

---

## Variable Declaration

### No Variable Shadowing

**Deviation:** Variables cannot be redeclared with the same name in nested scopes.

**JavaScript behavior:**
```javascript
let x = 1;
{
    let x = 2;      // Shadows outer x (allowed)
    console.log(x); // 2
}
console.log(x);     // 1
```

**Rhythm behavior:**
```javascript
let x = 1;
{
    let x = 2;      // Semantic validation error: "Variable 'x' already declared"
}
```

**Rationale:** Simplifies workflow state inspection and migration. When workflows suspend/resume, having unique variable names across all scopes makes state serialization and debugging clearer.

---

## Operators

### Nullish Coalescing (`??`) Semantics

**Deviation:** Matches JavaScript behavior (not a deviation, included for clarity)

**Behavior:** `??` returns the right operand only if the left is `null`, not for other falsy values.

```javascript
0 ?? 10;            // 0 (not 10)
"" ?? "default";    // "" (not "default")
false ?? true;      // false (not true)
null ?? 10;         // 10
```

This differs from logical OR (`||`) which checks for falsy values:
```javascript
0 || 10;            // 10
"" || "default";    // "default"
false || true;      // true
null || 10;         // 10
```

---

## Variable Scope

### Only `let` and `const` - No `var`

**Deviation:** The `var` keyword is not supported. Only `let` and `const` are available.

**Rationale:** `var` has confusing hoisting and function-scoped behavior that leads to bugs. Block-scoped `let`/`const` are clearer and safer for workflow execution.

---

## Notes

- Const enforcement happens at the semantic validation level, not at runtime
- Uninitialized declarations (`let x;`) default to `null`
- Redeclaration in the same scope is a semantic validation error
- For loop variables are scoped to the loop body
