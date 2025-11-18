# Architecture & Dependencies Analysis

## Current Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Rhythm Durable Execution                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Language Adapters (Python, Node.js, Rust)                    │
│  ├── Task Registration & Execution                            │
│  ├── Client API (start_workflow, queue_execution)             │
│  └── Worker Loop (poll, execute, report)                      │
│       │                                                         │
│       ├─ Adapter Layer (/core/src/adapter.rs) - STABLE API    │
│       │   └─ Single source of truth for FFI/WASM boundary     │
│       │                                                         │
│  ┌────▼──────────────────────────────────────────────────┐    │
│  │        Core Runtime (Rust) - rhythm_core              │    │
│  ├──────────────────────────────────────────────────────┤    │
│  │                                                       │    │
│  │  ┌─────────────────────────────────────────────────┐ │    │
│  │  │ Workflow Execution Engine                       │ │    │
│  │  ├─────────────────────────────────────────────────┤ │    │
│  │  │                                                 │ │    │
│  │  │  v2 Interpreter (ACTIVE - 344 KB)             │ │    │
│  │  │  ├── Parser (v2/parser/mod.rs) ✓              │ │    │
│  │  │  ├── Executor VM (v2/executor/mod.rs) ✓       │ │    │
│  │  │  ├── Standard Library (task, math) ✓          │ │    │
│  │  │  └── Tests (5K+ lines) ✓                      │ │    │
│  │  │                                                 │ │    │
│  │  │  v1 Interpreter (DEPRECATED - 259 KB) ✗       │ │    │
│  │  │  ├── Parser (pest-based, 2.8K lines)          │ │    │
│  │  │  ├── Executor (tree-walking, 505 lines)       │ │    │
│  │  │  ├── Tests (2K lines)                         │ │    │
│  │  │  └── Semantic Validator (630 lines)           │ │    │
│  │  │                                                 │ │    │
│  │  │  DECISION: Keep v2, Remove v1                 │ │    │
│  │  │                                                 │ │    │
│  │  └─────────────────────────────────────────────────┘ │    │
│  │                      ▲                                 │    │
│  │                      │ Workflow Registration           │    │
│  │                      │                                 │    │
│  │  ┌─────────────────────────────────────────────────┐ │    │
│  │  │ Execution Management (executions/)              │ │    │
│  │  ├─────────────────────────────────────────────────┤ │    │
│  │  │ • create_execution (idempotent)                 │ │    │
│  │  │ • claim_execution (worker polling)              │ │    │
│  │  │ • complete_execution (success + resume parent)  │ │    │
│  │  │ • fail_execution (error handling + retry)       │ │    │
│  │  │ • get_execution (status queries)                │ │    │
│  │  │ • list_executions (filtering)                   │ │    │
│  │  │ • cancel_execution                              │ │    │
│  │  └─────────────────────────────────────────────────┘ │    │
│  │                      ▲                                 │    │
│  │                      │ Task Enqueue/Complete           │    │
│  │                      │                                 │    │
│  │  ┌─────────────────────────────────────────────────┐ │    │
│  │  │ PostgreSQL Storage Layer (db/)                  │ │    │
│  │  ├─────────────────────────────────────────────────┤ │    │
│  │  │ Tables:                                         │ │    │
│  │  │  • executions (unified task/workflow)           │ │    │
│  │  │  • workflow_definitions                         │ │    │
│  │  │  • schema_migrations                            │ │    │
│  │  │                                                 │ │    │
│  │  │ Indexes: queue+status, parent_workflow, etc     │ │    │
│  │  │ Optimized for: polling, deduplication           │ │    │
│  │  │                                                 │ │    │
│  │  │ Total migrations: 14 files                      │ │    │
│  │  │ Evolution: Full → Simplified (Recent)           │ │    │
│  │  │                                                 │ │    │
│  │  │ Removed (recently):                             │ │    │
│  │  │  ✗ worker_heartbeats table                      │ │    │
│  │  │  ✗ workflow_signals table                       │ │    │
│  │  │  ✗ dead_letter_queue table                      │ │    │
│  │  │  ✗ checkpoint column                            │ │    │
│  │  │  ✗ worker_id, priority, timeout fields          │ │    │
│  │  │  ✗ args/kwargs (merged to inputs)               │ │    │
│  │  │  ✗ result/error (merged to output)              │ │    │
│  │  └─────────────────────────────────────────────────┘ │    │
│  │                                                       │    │
│  └───────────────────────────────────────────────────────┘    │
│                      ▲                                         │
│                      │                                         │
│       ┌──────────────┴──────────────┬──────────────────────┐   │
│       │                             │                      │   │
│  ┌────▼─────────┐         ┌─────────▼──────┐    ┌─────────▼──────┐
│  │ CLI Tools    │         │ Benchmarking   │    │ Config (409L)   │
│  │ ├ migrate    │         │ ├ WorkerMode   │    │ ├ File loading  │
│  │ ├ worker     │         │ ├ Metrics      │    │ ├ Env vars      │
│  │ ├ status     │         │ ├ Report       │    │ ├ Builder       │
│  │ ├ list       │         │ └─ 745 lines   │    │ ├ TOML parsing  │
│  │ ├ cancel     │         │    (OPTIONAL)  │    │ └─ CLI args     │
│  │ └ bench      │         │                │    │    (CAN        │
│  │    (OPTIONAL)│         │                │    │    SIMPLIFY)    │
│  └──────────────┘         └────────────────┘    └────────────────┘
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

Database ◄─────────────────────────────────────────────────────────► PostgreSQL
```

---

## Dependency Complexity Analysis

### Current Core Dependencies (Cargo.toml)
```rust
// Database & Async
sqlx            "0.7"    // SQL toolkit (MINIMAL - only Postgres needed)
tokio           "1.x"    // Async runtime (CORE - cannot remove)
tokio-postgres  "0.7"    // Postgres driver (ALTERNATIVE to sqlx, one could be removed)

// Serialization  
serde           "1.0"    // Serialization (CORE - needed for JSON/execution state)
serde_json      "1.0"    // JSON (CORE)

// Logging
tracing         "0.1"    // Structured logging (OPTIONAL - could use eprintln! for MVP)
tracing-subscriber      // Logging output (OPTIONAL)

// Parsing
pest            "2.8"    // Parser generator (ONLY for v1 - remove with v1)
pest_derive     "2.8"    // Macro (ONLY for v1)

// Utilities
uuid            "1.0"    // ID generation (CORE - used for executions)
chrono          "0.4"    // DateTime (CORE - used in timestamps)
sha2            "0.10"   // SHA256 (USED for workflow versioning)
anyhow          "1.0"    // Error handling (CORE)
thiserror       "1.0"    // Error macros (CORE)
clap            "4.5"    // CLI parsing (CORE for CLI, removable for lib only)
config          "0.14"   // Config files (OPTIONAL - for MVP could just use env)
toml            "0.8"    // TOML parsing (OPTIONAL - depends on config)
dotenvy         "0.15"   // .env loading (OPTIONAL - for MVP could skip)
libc            "0.2"    // System calls (UNUSED - possibly from old code?)
```

### Recommended Dependency Pruning

**REMOVE (with v1 interpreter):**
- `pest` 
- `pest_derive`

**CONSIDER REMOVING (post-MVP):**
- `tracing` + `tracing-subscriber` → Use simple `eprintln!` for MVP, add proper logging later
- `config` + `toml` + `dotenvy` → Use only `RHYTHM_DATABASE_URL` env var + CLI args

**KEEP (CORE):**
- `sqlx`, `tokio`, `tokio-postgres`
- `serde`, `serde_json`
- `uuid`, `chrono`
- `anyhow`, `thiserror`
- `clap` (useful for CLI)
- `sha2` (workflow versioning)

**INVESTIGATE:**
- `libc` - Check if actually used (may be dead dependency)

---

## Feature Matrix

### Core Features (MVP)
| Feature | Status | Complexity | Tests | Lines |
|---------|--------|-----------|-------|-------|
| Task execution | ✓ REQUIRED | Simple | 1000+ | 400 |
| Workflow orchestration | ✓ REQUIRED | Medium | 1000+ | 500 |
| Pause/Resume | ✓ REQUIRED | Complex | 500+ | 300 |
| Error handling | ✓ REQUIRED | Simple | 500+ | 200 |
| Database persistence | ✓ REQUIRED | Medium | 500+ | 300 |
| **TOTAL CORE** | | | **3500+** | **1700** |

### Optional/Advanced Features (Post-MVP)
| Feature | Status | Complexity | Tests | Lines |
|---------|--------|-----------|-------|-------|
| Benchmarking | ○ OPTIONAL | Medium | 0 | 745 |
| Signal coordination | ✗ REMOVED | - | - | - |
| Worker heartbeats | ✗ REMOVED | - | - | - |
| Distributed locking | ○ OPTIONAL | High | 0 | 0 |
| Dead-letter queue | ○ OPTIONAL | Medium | 0 | 0 |
| Advanced tracing | ○ OPTIONAL | Medium | 0 | 300+ |
| Multi-datacenter | ○ OPTIONAL | Very High | 0 | TBD |

---

## Code Debt & Technical Decisions

### Recent Cleanup (Past 20 commits)
✓ Removed worker heartbeats  
✓ Removed batch execution  
✓ Removed workflow signals  
✓ Unified args/kwargs → inputs  
✓ Unified result/error → output  
✓ Simplified schema  
✓ Cleaned up indexes  

**Evidence of:** Active refactoring toward simplification ✓

### Remaining Cleanup Opportunities
- Dual interpreter (v1 + v2) - **HIGH priority**
- Old interpreter tests - **HIGH priority** 
- Benchmark system - **MEDIUM priority**
- Config complexity - **MEDIUM priority**
- Dead dependencies - **LOW priority**

---

## Multi-Language SDK Architecture

```
Python SDK (14 files)              Node SDK (10 files)
├── Client API                     ├── Client API
│   └── start_workflow()           │   └── queueExecution()
├── Worker Loop                    ├── Worker Class
│   └── Worker class               │   └── polling loop
├── Task Decorators                ├── Task Registry
│   └── @task("name")              │   └── registerTask()
├── CLI (click)                    ├── CLI (commander.js)
│   ├── migrate                    │   ├── migrate
│   ├── worker                     │   ├── worker
│   ├── status                     │   ├── status
│   └── list                       │   └── list
├── RustBridge (FFI)               ├── RustBridge (N-API)
│   └── WASM calls                 │   └── Native bindings
└── Config                         └── Config

Both delegate to:
└─ Rust Core (/core/src/adapter.rs)
```

**Complexity Assessment:**
- Both SDKs are lean (14 + 10 files)
- Heavy lifting is in Rust core
- Keep SDKs as-is (they're simple)
- Only change core

---

## Test Coverage Analysis

### V1 Interpreter Tests (REMOVE)
```
Total: 1,969 lines
├── executor_tests.rs - 758 lines
└── expression_tests.rs - 1,211 lines (!)
    └── Tests for:
        - Arithmetic
        - Property access
        - Array operations
        - Type coercion
        - Edge cases (NaN, Infinity, etc.)
```

### V2 Executor Tests (KEEP & ENHANCE)
```
Total: 5,000+ lines
├── basic_tests.rs - 591 lines
├── assign_tests.rs - 514 lines
├── declare_tests.rs - 472 lines
├── operator_tests.rs - 632 lines
├── await_tests.rs
├── if_tests.rs
├── while_tests.rs
├── task_tests.rs
├── error_tests.rs
├── literal_tests.rs
├── optional_chaining_tests.rs
├── nullish_coalescing_tests.rs
└── stdlib_tests.rs

Plus:
├── parser/tests.rs - 1,585 lines
└── helpers.rs
```

**Verdict:** V2 tests are comprehensive and well-organized. V1 tests are extensive but redundant.

---

## Documentation Landscape

### Production Documentation (KEEP)
- README.md - Good overview
- WORKFLOW_DSL_FEATURES.md - Language reference
- FAQ.md - Common questions
- TECHNICAL_DEEP_DIVE.md - Architecture guide

### Exploratory/Reference (ARCHIVE)
42 files in `.context/` directory:
- ARCHITECTURE.md - Design notes
- DSL_WORKFLOW_IMPLEMENTATION.md - Implementation decisions
- REDIS_BACKEND_DESIGN.md - Rejected approach
- MARKET_RESEARCH.md - Competitive analysis
- SCOPED_VARIABLES_DESIGN.md - Feature exploration
- TESTING.md, TESTING_PRACTICES.md - Testing notes
- PERFORMANCE_*.md - Perf analysis
- MIGRATION.md - Schema evolution notes
- TODO.md - Old task list
- And 25+ more...

**Recommendation:** Archive to wiki or separate branch to reduce noise

---

## Proposed Simplified Architecture (Post-MVP)

```
┌─────────────────────────────────────────────────┐
│         Language Adapters (Py, Node, Rust)     │
├─────────────────────────────────────────────────┤
│                                                 │
│         Single Adapter API Layer                │
│         (simplified adapter.rs)                 │
│                 ▼                               │
│  ┌───────────────────────────────────────────┐ │
│  │  Lean Core Runtime (Rust)                 │ │
│  ├───────────────────────────────────────────┤ │
│  │                                           │ │
│  │  V2 Interpreter Only                      │ │
│  │  ├── Parser (maintained)                  │ │
│  │  ├── Executor (stack-based VM)            │ │
│  │  └── Stdlib (Task, Math)                  │ │
│  │                                           │ │
│  │  Execution Engine                         │ │
│  │  ├── Create/Claim/Complete/Fail          │ │
│  │  ├── Workflow registration                │ │
│  │  └── Error handling                       │ │
│  │                                           │ │
│  │  Database Layer                           │ │
│  │  ├── executions table                     │ │
│  │  ├── workflow_definitions                 │ │
│  │  └── Clean schema                         │ │
│  │                                           │ │
│  │  REMOVED:                                 │ │
│  │  ✗ V1 interpreter (2.5K lines)            │ │
│  │  ✗ V1 tests (2K lines)                    │ │
│  │  ✗ Benchmark system (745 lines)           │ │
│  │  ✗ Config file loading (200 lines)        │ │
│  │  ✗ Explorer documentation                 │ │
│  │                                           │ │
│  └───────────────────────────────────────────┘ │
│                    ▼                           │
│              PostgreSQL                        │
│                                                 │
└─────────────────────────────────────────────────┘

Savings: ~23-28% reduction in core code
Clarity: Single execution path
Maintenance: Reduced burden
```

---

## Migration Path

### Phase 1: Remove V1 Interpreter (No User-Facing Changes)
```
Day 1-2:  
  - Delete /core/src/interpreter/
  - Update /core/src/workflows.rs to use v2
  - Update imports in lib.rs
  - Run full test suite

Day 3:
  - Delete v1 tests
  - Audit adapter tests
  - Update language bindings (if needed)

Day 4:
  - Merge to main
  - Tag as "single-interpreter" version
```

**Result:** No changes to API, just internal cleanup

### Phase 2: Simplify Config (Optional)
```
If time allows:
  - Make DATABASE_URL env var required
  - Remove TOML config file support
  - Keep CLI arg override
  - Remove .env loading
  
Saves: 200 lines of code
```

### Phase 3: Archive Documentation
```
- Create wiki-branch or GitHub wiki
- Move .context/ files there
- Keep only README + live docs
```

**Low-risk operation, can be done anytime**

---

## Summary

**MVP Complexity Reduction Opportunities:**

| Category | Files | Lines | Effort | Risk | Impact |
|----------|-------|-------|--------|------|--------|
| V1 Interpreter + tests | 12 | 4.5K | HIGH | MEDIUM | HIGH ✓ |
| Benchmark system | 2 | 800 | MEDIUM | LOW | MEDIUM ○ |
| Config simplification | 1 | 200 | MEDIUM | LOW | LOW ○ |
| Doc archival | 42 | 500K | NONE | NONE | HIGH (clarity) ✓ |

**Recommended approach:**
1. ✓ **MUST DO:** Remove V1 interpreter (MVP blocker for clean code)
2. ○ **SHOULD DO:** Archive documentation (clarity)
3. ○ **NICE TO DO:** Simplify config (if time)
4. ○ **CAN SKIP:** Benchmark removal (keep if perf testing needed)

