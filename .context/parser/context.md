0) Goal & Scope (Phase 1)
We’re bootstrapping a new parser using PEST only, producing our existing AST types (already defined in the repo in executor_v2). This phase targets:
Statements + simple expressions only (no operator precedence yet).
No Pratt parser yet (that’s Phase 2).
No runtime/executor work here — just parse → AST → minimal semantic checks.
Fast, predictable, and easy to extend later.
Why this order
We can start running real scripts through the executor ASAP.
We avoid Pratt/precedence complexity until we need richer expressions.
The AST shape is stable; later we just swap the expression builder.
---
1) Surface Language (v1)
Statements (must support)
let / const declarations
Assignments (x = <expr>)
Expression statements (<expr>)
if (…) { … } else { … }
while (…) { … }
for (let i = …; …; …) { … } (basic C-style; no for..of/for..in yet) (this is still TODO in the executor / AST, it can be skipped here initially)
break, continue, return
try { … } catch (e) { … } finally { … }
await <expr> as:
a standalone expression statement, or
the RHS of an assignment
(These two placements only, by design, to simplify resume semantics.)
Expressions (simple, v1)
Literals: number (f64), string, boolean, null
Identifiers
Member access: a.b
Calls: f(a, b) and obj.m(a)
Arrays [ … ] and objects { key: value }
Parenthesized expression ( … )
> Not in v1: any operator precedence (+ - * / %, comparisons, && || ??, ternary), regex literals, template strings, destructuring, arrow functions, classes, this, var, hoisting, modules/imports, labels, generators.
Statement termination (no semicolons)
Newline-terminated by default.
A line continues if:
it ends with an open delimiter ( [ {, or
the next line starts with a continuation token: ., ?., (, [, or a binary operator (we have none in v1, but keep the hook), or comma , within list/args.
return/throw/break/continue must have their argument on the same line.
---
2) AST (reference only)
Use the existing Stmt / Expr enums in the repo.
Claude must not change their shape.
PEST → CST → AST builder constructs those exact types.
Each AST node should be assigned a Span (byte offsets + file ID). We keep a NodeId → Span map.
(We don’t paste the definitions here because they already exist and have evolved beyond what this doc knows. Claude must read them from the codebase.)
---
3) Spans & Positions
From PEST, capture byte spans for every matched rule (pair.as_span()).
Store absolute byte offsets on AST nodes. Don’t compute line/col here.
Build a line index per file (list of newline byte offsets) for later diagnostics; optional in this phase, but keep the spans.
Expressions: when handing a substring to any tokenizer (if used), offset token spans by the expression’s base start to keep absolute positions.
---
4) Grammar Strategy (PEST)
Author a .pest grammar that recognizes:
program, stmt, block, and each concrete statement rule.
expr_simple covering literals, identifiers, member access, calls, arrays, objects, parens.
Do not attempt to encode operator precedence in PEST.
For expression nesting, the grammar may be recursive (e.g., calls over members), but no infix operators yet.
Whitespace & comments
WHITESPACE includes spaces/tabs; treat newline distinctly to implement newline-terminated statements.
Support // line comments and /* … */ block comments.
---
5) CST → AST Builder
Walk Pair<Rule> trees:
Match on the rule, consume children, build the corresponding AST node.
Drop punctuation; normalize shapes (e.g., turn { k: v } into an AST map).
Attach Span to every AST node (or via NodeId → Span table).
No changes to the AST type definitions. If grammar makes something awkward, fix the grammar/builder, not the AST.
---
6) Minimal Semantic Validation (v1)
Run a small pass after AST build:
Scoping
Maintain a block scope stack for let/const.
No implicit globals — referencing an undeclared identifier is an error.
const must be initialized at declaration; re-assignment is an error.
Await placement (strict)
Allowed only as:
ExprStmt(Await(expr))
Assign(lhs, Await(expr))
Error on await anywhere else (e.g., inside object literal, conditional, or nested call args in this phase).
Control flow
break/continue only inside loops.
return only where your AST expects it (if functions aren’t in v1, reject).
No hoisting
Access before declaration in the same block is an error (simple TDZ).
Emit diagnostics with node spans; bail on errors.
---
7) Values & Operators (parser-facing notes)
Equality: language semantics will use strict equality; parser just recognizes tokens. No equality operator in v1 since operators are deferred.
Numbers: parse as f64.
Null vs undefined: only null exists.
---
8) Tests (must-have)
Golden AST tests: source → serde_json dump of AST; compare.
Negative tests: bad syntax, illegal await, implicit globals, duplicate let, invalid break, etc.
Span sanity: pick a few nodes and assert expected (start,end) ranges.
Keep fixtures tiny and focused.
---
9) Out of Scope (Phase 1)
Pratt/precedence parsing, all infix/unary operators.
Destructuring, arrow functions, classes, modules.
Regex/template strings.
Full-featured error pretty-printing (basic spans are enough now).
Any executor/runtime logic.
---
10) Phase 2 Preview (for future Claude runs)
Swap expr_simple consumption with a Pratt parser (tokenize expression spans, precedence table, associativity).
Keep AST unchanged where possible.
Expand semantic rules to allow await positions we green-light then.
---
11) Deliverables (for this task)
grammar/flow.pest (or similar)
parser/mod.rs (PEST wrapper)
builder.rs (CST → AST)
semantics.rs (minimal validation)
spans.rs (Span + line index helpers)
Tests under tests/parser_*
---
Hand-off Prompt for Claude
Title: Build PEST-only parser (statements + simple expressions) to our existing AST
Instructions for Claude (paste as-is):
> Read .context/parser/context.md (attached) end-to-end. Then open the repository and locate the existing AST type definitions (enums for Stmt, Expr, etc.). Do not change AST shapes.
Task: Implement a PEST-based parser that recognizes the Phase-1 surface described in the doc, builds the existing AST, and runs the minimal semantic validation pass.
Strict requirements:
1. Grammar: Author a .pest grammar covering statements/blocks and simple expressions only (literals, identifiers, member access, calls, arrays, objects, parens). No operator precedence in this phase.
2. CST → AST: Implement a builder that converts PEST Pair<Rule> trees into our existing AST types. Attach spans (absolute byte offsets) to every node via a map or field.
3. Newline termination: Implement newline-terminated statements with the continuation rules from the context doc.
4. Semantic checks: Add the minimal validation pass described (scopes for let/const, no implicit globals, await only as statement or assignment RHS, legal break/continue/return, no hoisting in block).
5. Errors: Return structured diagnostics including node spans. Human-pretty printing can be basic.
6. Tests: Provide golden AST tests and negative tests per the doc. Keep fixtures small and focused.
Don’ts:
Don’t introduce Pratt/precedence parsing yet.
Don’t modify AST enums or executor code.
Don’t add new language features beyond Phase 1.
Don’t silently “simplify” or “shortcut” unspecified behaviors — if something is unclear, ask.
Clarifications to ask before proceeding if uncertain:
Exact tokenization details for strings/numbers if something is ambiguous.
Edge cases in newline continuation (we’ll decide together).
How to encode spans (inline vs NodeId → Span table) given current AST ergonomics.
Deliverables:
grammar/flow.pest
parser/mod.rs (entry points)
parser/builder.rs (CST → AST)
parser/semantics.rs (minimal checks)
parser/spans.rs
tests/parser_* with golden + negative cases
After you wire it, run tests and share a summary of what’s covered, any ambiguities, and proposed follow-ups for Phase 2 (Pratt).
---
If you want, I can also add a tiny seed .pest scaffold and a couple of golden test examples to speed Claude up.