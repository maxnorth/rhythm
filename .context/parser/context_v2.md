# Parser Implementation Guide - Aligned with executor_v2

## 0) Goal & Scope (Phase 1)

Build a PEST-based parser that produces AST nodes compatible with `executor_v2`. This phase targets:
- **Statements**: All statement types currently implemented in executor_v2
- **Simple expressions**: Literals, identifiers, member access, calls, arrays, objects
- **No operator precedence yet** - operators are stdlib function calls (Phase 2 adds syntax sugar)
- **Minimal semantic validation** - scoping, const enforcement, await placement, control flow
- **Span tracking** - absolute byte offsets for error messages

**Why this order:**
- Get executor_v2 running with real parsed code ASAP
- Defer operator precedence complexity (Pratt parser) to Phase 2
- AST is already stable and battle-tested

---

## 1) Surface Language (v1)

### Statements (must support)

1. **Let/Const declarations**
   ```js
   let x = 42
   const y = "hello"
   let z  // uninitialized
   ```
   - Both map to `Stmt::Let { name, init }` in AST
   - Semantic validator tracks const vs let and enforces immutability

2. **Assignments** (including attribute assignment)
   ```js
   x = 100                    // Simple
   obj.prop = value           // Property assignment
   arr[0] = value             // Index assignment
   obj[key] = value           // Computed property
   obj.nested.deep = value    // Chained member access
   ```
   - Maps to `Stmt::Assign { var, path, value }`
   - `path` is `Vec<MemberAccess>` where `MemberAccess` is `Prop | Index`

3. **Expression statements**
   ```js
   foo()
   Task.run("taskName", args)
   ```
   - Maps to `Stmt::Expr { expr }`

4. **If/else**
   ```js
   if (condition) {
       // then branch
   } else {
       // else branch (optional)
   }
   ```
   - Maps to `Stmt::If { test, then_s, else_s }`
   - No `else if` sugar yet - parser produces nested If in else branch

5. **While loops**
   ```js
   while (condition) {
       // body
   }
   ```
   - Maps to `Stmt::While { test, body }`

6. **For loops** (for-in style)
   ```js
   for (let item in items) {
       // body
   }
   ```
   - Maps to `Stmt::For { iterator, iterable, body }`
   - C-style `for(init; test; update)` not supported yet

7. **Break/Continue**
   ```js
   break
   continue
   ```
   - Maps to `Stmt::Break` and `Stmt::Continue`
   - No label support in Phase 1 (AST has it, grammar doesn't)

8. **Return**
   ```js
   return
   return value
   ```
   - Maps to `Stmt::Return { value }`

9. **Try/Catch** (no finally yet)
   ```js
   try {
       // body
   } catch (e) {
       // catch body
   }
   ```
   - Maps to `Stmt::Try { body, catch_var, catch_body }`
   - No `finally` clause in Phase 1

10. **Await** (strict placement)
    ```js
    await expr              // As statement
    let x = await expr      // As assignment RHS
    ```
    - Only allowed in these two positions
    - Error on `await` anywhere else (nested calls, conditionals, etc.)

11. **Blocks**
    ```js
    {
        stmt1
        stmt2
    }
    ```
    - Maps to `Stmt::Block { body }`
    - Used implicitly for if/while/for bodies

---

### Expressions (simple, v1)

Current executor_v2 `Expr` enum:
```rust
pub enum Expr {
    LitBool { v: bool },
    LitNum { v: f64 },
    LitStr { v: String },
    LitList { elements: Vec<Expr> },
    LitObj { properties: Vec<(String, Expr)> },
    Ident { name: String },
    Member { object: Box<Expr>, property: String },  // Only .prop, not [index]
    Call { callee: Box<Expr>, args: Vec<Expr> },
    Await { inner: Box<Expr> },
}
```

**Supported in Phase 1:**

1. **Literals**
   - Numbers: `42`, `3.14`, `-10`, `1.5e10`
   - Strings: `"double"`, `'single'` (no template literals)
   - Booleans: `true`, `false`
   - Null: `null`

2. **Identifiers**
   - `varName`, `_private`, `camelCase`

3. **Member access** (property only)
   - `obj.prop`
   - `obj.nested.deep`
   - **NOT YET**: `arr[0]` - requires AST extension (see below)

4. **Function calls**
   - `foo()`
   - `foo(a, b, c)`
   - `obj.method(args)`
   - `add(x, y)` - operators as function calls

5. **Arrays**
   - `[]`, `[1, 2, 3]`, `[x, y, nested()]`

6. **Objects**
   - `{}`, `{ key: value }`, `{ a: 1, b: nested() }`
   - Keys must be identifiers or strings

7. **Parenthesized expressions**
   - `(expr)`

8. **Await expressions**
   - `await foo()`
   - Validated to only appear as statement or assignment RHS

**NOT in Phase 1:**
- Infix operators (`+`, `-`, `*`, `/`, `<`, `==`, etc.) - use function calls instead
  - `x + y` → `add(x, y)`
  - `x < y` → `lt(x, y)`
  - `x == y` → `eq(x, y)`
- Logical operators (`&&`, `||`, `!`) - use functions if needed
- Ternary operator (`? :`)
- Unary operators (`!`, `-`, `+`, `typeof`)
- Template strings `` `hello ${name}` ``
- Regex literals `/pattern/`
- Arrow functions `() => {}`
- Destructuring `{ a, b } = obj`
- Spread `...args`
- Optional chaining `obj?.prop`
- Nullish coalescing `??`

---

### Statement Termination (newline-based)

**Rules:**
- Statements are terminated by newlines by default
- A line continues if:
  - It ends with an open delimiter: `(`, `[`, `{`
  - The next line starts with: `.`, `(`, `[`, `,` (within lists/args)
  - Inside a function call or array literal
- `return`, `break`, `continue` arguments must be on same line

**Examples:**
```js
// Valid - newline terminates
let x = 42
let y = 100

// Valid - open delimiter continues
let arr = [
    1,
    2,
    3
]

// Valid - member access continues
let result = obj
    .method()
    .chain()

// INVALID - return argument on next line
return
    value  // ERROR: return already terminated

// Valid - return on same line
return value
```

**No semicolons required** - simpler than JavaScript's ASI rules.

---

## 2) AST (reference)

**Use the existing AST** from `core/src/interpreter/executor_v2/types/ast.rs`.

**DO NOT modify these types:**
```rust
pub enum Stmt { /* ... */ }
pub enum Expr { /* ... */ }
pub enum MemberAccess { Prop { property: String }, Index { expr: Expr } }
```

**Parser responsibility:**
- Transform PEST CST → existing AST types
- Attach spans (byte offsets) to nodes
- No changes to enum shapes

---

## 3) Known AST Limitations & Workarounds

### Issue 1: Index Access Not Supported

**Problem:** `Expr::Member` only has `property: String`, can't represent `arr[0]` or `obj[key]`

**Short-term workaround for Phase 1:**
Parse `arr[index]` as `Call(Member(arr, "[]"), [index])` - hacky but works.

**Phase 2 solution (requires AST change):**
Add `Expr::Index { object: Box<Expr>, index: Box<Expr> }`

### Issue 2: Else-if chains

**Problem:** No dedicated `else if` node

**Solution:** Parse as nested If:
```js
if (a) { ... }
else if (b) { ... }
else { ... }

// Becomes:
If {
    test: a,
    then_s: ...,
    else_s: Some(If { test: b, then_s: ..., else_s: ... })
}
```

### Issue 3: Block wrapping

**Problem:** If/while/for bodies expect `Box<Stmt>`, but grammar allows multiple statements in `{}`

**Solution:** Parser wraps statement lists in `Stmt::Block { body: Vec<Stmt> }`

---

## 4) Spans & Positions

**From PEST:**
- Capture `pair.as_span()` for every rule
- Store absolute byte offsets `(start, end)`

**Storage strategy:**
- **Option A**: Add `span: Span` field to every AST node (requires AST changes)
- **Option B**: Separate `NodeId → Span` map (doesn't modify AST)

**Recommendation**: Use Option B for Phase 1 since we can't change AST.

**Line index:**
- Build per-file: `Vec<usize>` of newline byte offsets
- Used for error messages (byte → line/col conversion)

---

## 5) Grammar Strategy (PEST)

### High-level structure

```pest
program = { SOI ~ statement* ~ EOI }

statement = {
    let_stmt
    | const_stmt
    | assign_stmt
    | if_stmt
    | while_stmt
    | for_stmt
    | try_stmt
    | return_stmt
    | break_stmt
    | continue_stmt
    | expr_stmt
}

expression = {
    await_expr
    | call_expr
    | member_expr
    | array_expr
    | object_expr
    | paren_expr
    | literal
    | identifier
}
```

### Key grammar rules

1. **Assignment with member access**
   ```pest
   assign_stmt = { identifier ~ member_accessor* ~ "=" ~ expression }
   member_accessor = { ("." ~ identifier) | ("[" ~ expression ~ "]") }
   ```

2. **While loops**
   ```pest
   while_stmt = { "while" ~ "(" ~ expression ~ ")" ~ block }
   ```

3. **Try/catch**
   ```pest
   try_stmt = {
       "try" ~ block ~
       "catch" ~ "(" ~ identifier ~ ")" ~ block
   }
   ```

4. **For-in loops**
   ```pest
   for_stmt = { "for" ~ "(" ~ "let" ~ identifier ~ "in" ~ expression ~ ")" ~ block }
   ```

### Whitespace & comments

```pest
WHITESPACE = _{ " " | "\t" }  // NOT newline - we need it for termination
NEWLINE = _{ "\n" | "\r\n" }
COMMENT = _{
    "//" ~ (!NEWLINE ~ ANY)* ~ NEWLINE? |  // Line comment
    "/*" ~ (!"*/" ~ ANY)* ~ "*/"           // Block comment
}
```

**Newline handling:**
- Track newlines explicitly to implement statement termination rules
- Use PEST's `@` (atomic) and `$` (compound-atomic) carefully
- May need custom lexer-like rules for continuation logic

---

## 6) CST → AST Builder

**Pattern:**
```rust
fn build_stmt(pair: Pair<Rule>) -> Result<Stmt> {
    match pair.as_rule() {
        Rule::let_stmt => {
            // Extract name, init
            // Build Stmt::Let { name, init }
        }
        Rule::assign_stmt => {
            // Extract var, path, value
            // Build Stmt::Assign { var, path, value }
        }
        // ... etc
    }
}
```

**Key principles:**
- Drop punctuation tokens
- Normalize shapes (e.g., object literal → Vec<(String, Expr)>)
- Capture spans for every node
- No AST modifications

---

## 7) Minimal Semantic Validation

### Scoping rules

**Track let/const:**
```rust
struct Scope {
    vars: HashMap<String, VarInfo>,
    parent: Option<Box<Scope>>,
}

struct VarInfo {
    is_const: bool,
    declared_at: Span,
}
```

**Validation:**
- Reference to undeclared identifier → error
- Re-declaration in same scope → error
- Const reassignment → error
- Access before declaration (TDZ) → error

### Await placement

**Allowed:**
- `Stmt::Expr { expr: Expr::Await { ... } }`
- `Stmt::Assign { value: Expr::Await { ... } }`

**Error everywhere else:**
- Inside function call args: `foo(await x)` ❌
- Inside array: `[await x]` ❌
- Inside object: `{ k: await x }` ❌
- Inside conditional: `if (await x)` ❌

**Implementation:** Walk AST, track context depth, error if await in non-allowed position.

### Control flow

**Break/continue:**
- Only inside `while` or `for` loops
- Track loop depth, error if depth == 0

**Return:**
- Phase 1: Allow anywhere (top-level scripts act like functions)
- Phase 2: Only inside function bodies (when functions are added)

---

## 8) Operators as Function Calls

**Phase 1 approach:**
All operators are stdlib function calls:

```js
// User writes:
if (eq(x, 10)) {
    y = add(y, 1)
}

// Instead of:
if (x == 10) {
    y = y + 1
}
```

**Available operators:**
- Arithmetic: `add(a,b)`, `sub(a,b)`, `mul(a,b)`, `div(a,b)`
- Comparison: `eq(a,b)`, `lt(a,b)`, `lte(a,b)`, `gt(a,b)`, `gte(a,b)`

**Phase 2 syntax sugar:**
Parser will transform `x + y` → `Call(Ident("add"), [x, y])` using Pratt parser.

**Why this order:**
- Gets parser working immediately
- Executor already has these as stdlib functions
- Adding syntax sugar later doesn't break anything

---

## 9) Tests (must-have)

### Golden AST tests
```rust
#[test]
fn test_let_statement() {
    let src = "let x = 42";
    let ast = parse(src).unwrap();
    let json = serde_json::to_string_pretty(&ast).unwrap();
    assert_snapshot!(json);
}
```

### Negative tests
```rust
#[test]
fn test_implicit_global_error() {
    let src = "x = 10";  // x not declared
    let err = parse(src).unwrap_err();
    assert!(err.contains("undefined variable 'x'"));
}

#[test]
fn test_const_reassignment_error() {
    let src = "const x = 1\nx = 2";
    let err = parse(src).unwrap_err();
    assert!(err.contains("cannot reassign const"));
}

#[test]
fn test_await_in_call_args_error() {
    let src = "foo(await bar())";
    let err = parse(src).unwrap_err();
    assert!(err.contains("await not allowed"));
}
```

### Span tests
```rust
#[test]
fn test_spans_are_accurate() {
    let src = "let x = 42";
    let ast = parse_with_spans(src).unwrap();
    let let_stmt_span = get_span(&ast, node_id);
    assert_eq!(let_stmt_span, Span { start: 0, end: 10 });
}
```

---

## 10) Out of Scope (Phase 1)

- Pratt/precedence parsing (all operators)
- Infix/unary operator syntax
- Index access `arr[i]` as expression (AST limitation)
- Loop labels
- Destructuring
- Arrow functions
- Classes
- Modules/imports
- `this` keyword
- Regex/template literals
- Optional chaining `?.`
- Nullish coalescing `??`
- For-of / for-in distinction
- Finally clause in try/catch

---

## 11) Phase 2 Preview

**What changes:**
1. Add Pratt parser for operator precedence
2. Parse `x + y` → `Call(Ident("add"), [x, y])`
3. Add `Expr::Index` to AST or transform to Call hack
4. Loosen await restrictions (allow in more places)
5. Add template string support
6. Add logical operators (`&&`, `||`, `!`)

**AST stays mostly the same** - operators are still function calls internally.

---

## 12) Deliverables

```
core/src/parser_v2/
├── mod.rs              # Public API, parse() entry point
├── grammar.pest        # PEST grammar
├── builder.rs          # CST → AST transformation
├── semantics.rs        # Validation pass (scoping, await, control flow)
├── spans.rs            # Span tracking, line index
└── tests/
    ├── golden/         # AST snapshot tests
    ├── negative/       # Error tests
    └── spans/          # Span accuracy tests
```

---

## 13) Implementation Checklist

### Phase 1a: Basic statements (no control flow)
- [ ] PEST grammar skeleton
- [ ] Let/const statements
- [ ] Simple assignment (`x = value`)
- [ ] Expression statements
- [ ] Return statements
- [ ] Literals (num, str, bool, null)
- [ ] Identifiers
- [ ] Basic tests

### Phase 1b: Complex expressions
- [ ] Member access (`obj.prop`)
- [ ] Function calls
- [ ] Arrays `[...]`
- [ ] Objects `{...}`
- [ ] Await expressions
- [ ] Tests for expressions

### Phase 1c: Control flow
- [ ] If/else statements
- [ ] While loops
- [ ] For-in loops
- [ ] Break/continue
- [ ] Try/catch
- [ ] Tests for control flow

### Phase 1d: Advanced assignment
- [ ] Attribute assignment (`obj.prop = value`)
- [ ] Index assignment workaround (`obj["key"] = value`)
- [ ] Chained member access
- [ ] Tests for complex assignment

### Phase 1e: Semantic validation
- [ ] Scope tracking (let/const)
- [ ] Const immutability enforcement
- [ ] Await placement validation
- [ ] Break/continue in loops only
- [ ] Negative tests

### Phase 1f: Spans & errors
- [ ] Span tracking for all nodes
- [ ] Line index for error messages
- [ ] Pretty error formatting
- [ ] Span accuracy tests

---

## 14) Key Differences from Original Context.md

**Added to Phase 1:**
- ✅ While loops (executor has them)
- ✅ Try/catch (executor has them)
- ✅ Attribute assignment `obj.prop = value` (executor has it)
- ✅ For-in loops (AST ready)

**Clarified:**
- ✅ Operators are function calls, not syntax (matches executor)
- ✅ No `else if` sugar - nested If nodes
- ✅ Index access `arr[i]` requires AST extension or workaround
- ✅ Let vs const: both map to `Stmt::Let`, validator tracks immutability

**Still deferred to Phase 2:**
- ❌ Operator precedence (Pratt parser)
- ❌ Template strings
- ❌ Loop labels (AST ready, grammar not)
- ❌ Finally clause

---

## 15) Parser API

```rust
// Entry point
pub fn parse(source: &str) -> Result<Stmt, ParseError> {
    // 1. PEST parse → CST
    // 2. Build AST
    // 3. Validate semantics
    // 4. Return root Stmt (usually Block)
}

// With spans
pub fn parse_with_spans(source: &str) -> Result<(Stmt, SpanMap), ParseError> {
    // Same as above, but also returns NodeId → Span map
}

// Error type
pub struct ParseError {
    pub message: String,
    pub span: Span,
    pub kind: ErrorKind,
}

pub enum ErrorKind {
    SyntaxError,
    UndefinedVariable,
    ConstReassignment,
    InvalidAwait,
    InvalidBreak,
    // ...
}
```

---

**This document supersedes context.md and reflects the current state of executor_v2.**
