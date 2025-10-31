# Testing Practices for Rhythm

> Guidelines and lessons learned for writing effective tests in a multi-language FFI architecture

## Core Principle: FFI Interfaces Drive Design

**The adapter interface (FFI) takes priority over test convenience.**

Tests must adapt to the interface required by language adapters (Python, Node.js), even if it results in less idiomatic or slightly verbose Rust test code.

### Why This Matters

Rhythm's architecture has three layers:
1. **Rust Core** - Performance-critical operations
2. **FFI Boundary** - Bridge between Rust and language adapters
3. **Language Adapters** - Python, Node.js, etc.

The FFI boundary is the **contract** between layers. Breaking it to make tests cleaner creates technical debt and confuses the architecture.

---

## Lesson: Don't Change FFI Interfaces for Tests

### ❌ Anti-Pattern: Adapting Interface to Tests

```rust
// Original FFI interface - accepts JsonValue
pub async fn fail_execution(execution_id: &str, error: JsonValue, retry: bool) -> Result<()> {
    // ... implementation
}

// ❌ WRONG: Changed signature for test convenience
pub async fn fail_execution(execution_id: &str, error: &str, retry: bool) -> Result<()> {
    let error_json = serde_json::json!({"error": error});
    fail_execution_json(execution_id, error_json, retry).await
}

// Now need a separate function for the real interface
pub async fn fail_execution_json(execution_id: &str, error: JsonValue, retry: bool) -> Result<()> {
    // ... actual implementation
}

// Tests look clean but interface is backwards
#[test]
async fn test_fail() {
    fail_execution(&id, "Network error", false).await.unwrap();
}
```

**Problems:**
- FFI now calls `fail_execution_json` instead of `fail_execution`
- The "real" function has an ugly name
- Creates confusion about which function is the canonical API
- Tests dictate production code design (tail wagging the dog)

### ✅ Correct Pattern: Tests Adapt to Interface

```rust
// Keep the FFI interface as the primary function
pub async fn fail_execution(execution_id: &str, error: JsonValue, retry: bool) -> Result<()> {
    // ... implementation
}

// Tests use the real interface, even if slightly verbose
#[test]
async fn test_fail() {
    fail_execution(
        &id,
        serde_json::json!({"error": "Network error"}),
        false
    ).await.unwrap();
}
```

**Benefits:**
- ✅ FFI interface is clean and obvious
- ✅ Single source of truth for the function
- ✅ Tests validate the actual API consumers use
- ✅ No confusion about which function to call

---

## Guidelines

### 1. FFI-First Design

When adding or modifying functions used across the FFI boundary:

1. **Start with the FFI requirements** - What do Python/Node.js need?
2. **Design the Rust signature** - Match FFI needs (e.g., `String`, `JsonValue`)
3. **Write tests using that signature** - Even if verbose

### 2. When Test Convenience Conflicts with FFI

If a test would be cleaner with a different signature:

**Option A: Accept slightly verbose tests**
```rust
// Test uses the real interface
fail_execution(&id, serde_json::json!({"error": msg}), false).await
```

**Option B: Helper functions in tests (not production code)**
```rust
#[cfg(test)]
mod test_helpers {
    use super::*;

    pub async fn fail_execution_str(id: &str, error: &str, retry: bool) -> Result<()> {
        fail_execution(id, serde_json::json!({"error": error}), retry).await
    }
}

#[test]
async fn test_fail() {
    test_helpers::fail_execution_str(&id, "Network error", false).await.unwrap();
}
```

**Never do:**
- ❌ Change the public API signature for test convenience
- ❌ Create a `_json` variant when the main function should accept JSON
- ❌ Make the FFI call a secondary function

### 3. Acceptable Test-Only Functions

Test helpers are fine when they:
- Live in `#[cfg(test)]` blocks or `tests.rs`
- Don't change the public API
- Provide convenience without altering contracts

```rust
#[cfg(test)]
mod test_utils {
    pub fn make_test_params(external_id: Option<String>) -> CreateExecutionParams {
        CreateExecutionParams {
            exec_type: ExecutionType::Task,
            function_name: "test.task".to_string(),
            queue: "test".to_string(),
            priority: 5,
            args: serde_json::json!([]),
            kwargs: serde_json::json!({}),
            max_retries: 3,
            timeout_seconds: Some(300),
            parent_workflow_id: None,
            external_id,
        }
    }
}
```

---

## Rationale: Why JsonValue at FFI Boundary?

Language adapters send JSON over FFI because:
- **Type safety across languages** - JSON is universal
- **Flexibility** - No need to define every error structure in Rust
- **Performance** - Single serialization, not string → JSON → struct
- **Simplicity** - Python can send any dict, Rust validates

Even though Rust would prefer typed structs, the FFI contract uses `JsonValue` for these reasons. Tests must respect this.

---

## Red Flags in Code Review

Watch for these patterns that suggest tests are driving interface design:

- ❌ Function suffixes like `_json`, `_raw`, `_internal` on the "real" implementation
- ❌ FFI calling anything other than the main public function
- ❌ Comments like "for tests" or "convenience wrapper" on public APIs
- ❌ Multiple functions doing the same thing with different signatures
- ❌ Tests using a simpler interface than FFI uses

---

## Summary

**Core Rule:** The FFI interface is the contract. Tests validate the contract, not convenience variants.

**When in doubt:**
1. What does the language adapter (Python/Node) need?
2. Design the Rust function for that use case
3. Tests use that same interface
4. If tests are verbose, that's okay - they're testing the real thing

**Remember:** A slightly verbose test that validates the real interface is better than a clean test that validates a fake interface.
