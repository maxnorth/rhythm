# Exploratory Testing Results

## Summary

Performed extensive exploratory testing of the workflow DSL parser to find edge cases and potential bugs.

**Result**: Found and fixed **2 bugs** (escape sequence handling, number parsing), added **enhanced number formats**, **unquoted key support**, **line validation**, and **40+ edge case tests**.

## Bugs Found & Fixed

### ❌ Bug 1: Escape Sequences Not Processed

**Issue**: Escaped characters in strings were not being unescaped.

```flow
task("t", { "msg": "He said \"hello\"" })
```

**Expected**: `msg` should contain: `He said "hello"`
**Actual (before fix)**: `msg` contained: `He said \"hello\"`

**Root Cause**: The `parse_string()` function was only stripping quotes but not processing escape sequences like `\"`, `\\`, `\n`, etc.

**Fix**: Updated `parse_string()` to handle common escape sequences:
- `\"` → `"`
- `\\` → `\`
- `\n` → newline
- `\t` → tab
- `\r` → carriage return
- `\'` → `'`

**Test**: [parser.rs:613](core/src/interpreter/parser.rs#L613) `test_edge_escaped_chars_in_strings`

### ❌ Bug 2: Numbers Starting/Ending with Decimal Point Not Supported

**Issue**: Numbers like `.5` (0.5) and `5.` (5.0) were not being parsed correctly.

```flow
task("t", { "val": .5 })    # Was failing
task("t", { "val": 5. })    # Was failing
task("t", { "val": -.5 })   # Was failing
```

**Expected**: These are valid number formats (shorthand for 0.5, 5.0, -0.5)
**Actual (before fix)**: Parser rejected them

**Root Cause**: The `number` rule in the Pest grammar required at least one digit before the decimal point.

**Fix**: Updated the `number` rule in [workflow.pest:76](core/src/interpreter/workflow.pest#L76) to support:
- Numbers starting with dot: `.456`
- Numbers ending with dot: `123.`
- Negative versions: `-.456`

**Tests**:
- `test_edge_number_starting_with_dot`
- `test_edge_number_ending_with_dot`
- `test_edge_negative_number_with_dot`

## Features Added (40+ edge case tests)

### ✅ Syntax That Works

1. **Empty task name** - `task("", {})`
2. **Special characters in task names** - dashes, dots, slashes, colons
3. **Unicode in task names** - emoji 😀, Chinese characters 任务
4. **Negative numbers** - `-42`, `-3.14`
5. **Scientific notation** - `1e10`, `1e-10`, `1.5E+5`
6. **Escaped characters** - `\"`, `\\`, `\n`, `\t`, `\r`
7. **Empty arrays** - `[]`
8. **Empty objects** - `{}`
9. **Deeply nested JSON** - 6 levels deep works fine
10. **Variables in arrays** - `[var1, var2, var3]`
11. **Variables in nested structures** - Works at any depth
12. **Variable shadowing** - Same variable name assigned multiple times
13. **Reserved words as variables** - `let class`, `let import`, `let return`
14. **No spaces** - `let x=task("t",{})`
15. **Tabs as whitespace** - `let\tx\t=\ttask("t",{})`
16. **Trailing comments** - `task("t", {}) # comment`
17. **Windows line endings** - `\r\n` works
18. **Empty workflow** - Empty file is valid
19. **Only comments** - File with only comments is valid
20. **Mixed quote types** - `{ "key1": 'value1', 'key2': "value2" }`
21. **Multiline JSON** - Newlines inside JSON objects
22. **Task without inputs** - `task("task_name")` defaults to `{}`
23. **Zero values** - `0` and `0.0`
24. **Very large numbers** - Handled as scientific notation (1e21)
25. **Trailing commas** - Both in objects and arrays (parser accepts)
26. **Bare identifier variables** - `{ val: my_var }` → `$my_var`
27. **Numbers starting/ending with dot** - `.5`, `5.`, `-.5` all work
28. **Hex numbers** - `0x1234`, `0xFF`, `0xDEAD_BEEF` with underscores
29. **Binary numbers** - `0b1010`, `0b11110000` with underscores
30. **Numbers with underscores** - `1_000_000`, `3.14_15_92`
31. **Unquoted keys** - `{ userId: 123, name: "Alice" }`
32. **Mixed quoted/unquoted keys** - `{ userId: 123, "is-admin": false }`
33. **Nested unquoted keys** - Works at any depth
34. **Unquoted keys with variables** - `{ user: userId, data: userData }`

### ❌ Syntax That (Correctly) Fails

1. **Consecutive commas** - `{ "a": 1,, "b": 2 }` fails
2. **Comments inside JSON** - `{ "a": 1 /* comment */ }` fails
3. **Multiple statements on one line** - `task("t1", {}) task("t2", {})` fails (enforced via line validation)
4. **Semicolons** - `task("t", {});` fails (explicitly disallowed, unlike JavaScript)
5. **Hash comments** - `task("t", {}) # comment` fails (use `//` instead)

## Test Coverage

**Total Tests: 74** (27 original + 47 edge cases)

### Parser Tests: 63
- Basic workflow parsing
- Variable assignment & references
- All JSON types (string, number, boolean, null, array, object)
- Escape sequences
- Comments & whitespace
- Enhanced number formats (hex, binary, underscores, flexible decimals)
- Unquoted keys
- Line validation (no multiple statements per line)
- Semicolon validation (explicitly disallowed)
- Hash comment validation (use // instead)
- Edge cases (47 tests)

### Executor Tests: 11
- Variable resolution in all contexts
- Missing variable handling
- Complex nested resolution
- All primitive types

## Interesting Findings

1. **Trailing commas** - Our parser accepts them even though they're not standard JSON. This is because we use Pest's optional comma pattern. Could be a feature or a bug depending on perspective.

2. **Multiple statements on one line** - Initially worked due to whitespace-based parsing, but now **enforced via line number validation** in the Rust parser. Attempting to put multiple statements on the same line will result in a clear error message.

3. **Semicolons explicitly disallowed** - Unlike JavaScript which makes semicolons optional, the Currant DSL **prohibits semicolons entirely**. This enforces a clean, Python-like syntax where newlines are the statement separators.

4. **Comments use `//` not `#`** - Unlike Python/Ruby/Bash which use `#`, the Currant DSL uses `//` for comments (similar to JavaScript, Rust, C++). Hash symbols `#` inside strings are allowed, but `#` comments outside strings are rejected with a helpful error message.

5. **Large number handling** - Very large integers automatically converted to scientific notation by serde_json (e.g., `999999999999999999999` → `1e21`).

6. **Escape sequences** - Initially broken, now fixed. Common escapes (`\"`, `\\`, `\n`, `\t`, `\r`) now work correctly.

7. **Reserved words** - JavaScript/Python reserved words like `class`, `import`, `return` can be used as variable names without issues.

8. **Unicode support** - Full Unicode support in task names and string values (emoji, CJK characters, etc.).

9. **Number formats** - Parser now supports hex (`0xFF`), binary (`0b1010`), underscores (`1_000_000`), and flexible decimals (`.5`, `5.`) - going well beyond standard JSON to provide a better developer experience.

10. **Unquoted keys** - JSON5-style syntax allows cleaner object notation: `{ userId: 123 }` instead of `{ "userId": 123 }`.

## Tests Added

All edge case tests are in [parser.rs](core/src/interpreter/parser.rs) starting at line 528, marked with the `test_edge_` prefix.

Run edge case tests:
```bash
cargo test --lib test_edge_
```

Run all interpreter tests:
```bash
cargo test --lib interpreter
```

## Recommendations

1. **Escape sequences are now working** - No action needed.

2. **Trailing commas** - Currently accepted. If strict JSON compliance is desired, the grammar could be updated to reject them.

3. **Multiple statements per line** - Now enforced via line validation. Each statement must be on its own line.

4. **Semicolons** - Explicitly disallowed (unlike JavaScript). Clean, Python-like newline-based syntax.

5. **Hash comments** - Explicitly disallowed. Use `//` for comments (JavaScript/Rust-style).

6. **Number formats** - Full support for hex, binary, underscores, and flexible decimals. Goes beyond standard JSON to provide better DX.

7. **Unquoted keys** - Implemented and working. Provides cleaner, more readable syntax.

## Conclusion

The parser is robust and handles a wide variety of edge cases correctly. Two bugs found (escape sequences and number parsing) have been fixed. Enhanced features added (hex/binary numbers, underscores, unquoted keys, line validation, semicolon prohibition, `//` comments). All **74 tests now pass**.

The workflow DSL now provides a **modern, developer-friendly syntax** with clean, Python-like statement separation (newlines, not semicolons) and JavaScript/Rust-style `//` comments, while maintaining correctness and proper validation. Great collaboration on identifying what should vs. shouldn't work!
