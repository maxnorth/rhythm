# Workflow DSL Feature Reference

Complete reference for the Currant workflow DSL syntax and features.

## Overview

The Currant workflow DSL is a clean, expressive language for defining task workflows. It combines the best of JSON with modern programming language conveniences.

**Philosophy**: Strict where it matters, flexible where it helps.

## Syntax Features

### 1. Task Execution

```flow
// Awaited task - blocks until complete
await task("process_order", { orderId: 123 })

// Fire-and-forget - queues and continues immediately
task("send_analytics", { event: "purchase" })

// Task without inputs
task("cleanup")
```

### 2. Variable Assignment

```flow
// Capture task result
let orderId = await task("create_order", { amount: 100 })

// Use variable in subsequent tasks
await task("charge_payment", { orderId: orderId })

// Fire-and-forget with assignment
let logId = task("log_event", { msg: "started" })
```

### 3. JSON5-Style Object Syntax

**Unquoted Keys** - Much cleaner than standard JSON:
```flow
// Traditional JSON (still supported)
task("update", { "user-id": 123, "name": "Alice" })

// Modern style with unquoted keys
task("update", { userId: 123, name: "Alice" })

// Mix and match as needed
task("update", { userId: 123, "is-admin": false })
```

**Variable References** - Bare identifiers:
```flow
let userId = await task("authenticate", {})

// Use variable without quotes
task("fetch_profile", { userId: userId })

// Variables work anywhere
task("process", {
  user: userId,
  settings: {
    notifyUser: userId,
    preferences: userPrefs
  }
})
```

### 4. Enhanced Number Formats

**Hexadecimal** - Great for colors, addresses, bit masks:
```flow
task("set_color", {
  color: 0xFF5733,
  mask: 0xDEAD_BEEF
})
```

**Binary** - Perfect for bit flags:
```flow
task("set_permissions", {
  flags: 0b1010,
  mask: 0b11110000
})
```

**Underscores** - Improves readability:
```flow
task("process_payment", {
  amount: 1_000_000,
  precision: 3.14_15_92,
  addr: 0xDEAD_BEEF
})
```

**Flexible Decimals** - Leading/trailing dots:
```flow
task("calculate", {
  small: .5,      # Same as 0.5
  large: 5.,      # Same as 5.0
  negative: -.25  # Same as -0.25
})
```

**Scientific Notation**:
```flow
task("physics", {
  huge: 1e10,
  tiny: 1.5E-8,
  avogadro: 6.022e23
})
```

### 5. String Features

**Escape Sequences**:
```flow
task("format", {
  msg: "Line 1\nLine 2\tTabbed",
  path: "C:\\Users\\data",
  quote: "He said \"hello\""
})
```

**Single or Double Quotes**:
```flow
task("test", {
  msg1: "double quotes",
  msg2: 'single quotes'
})
```

**Unicode**:
```flow
task("😀", { msg: "任务完成" })  # Emoji and CJK characters supported
```

### 6. Complex Data Structures

**Nested Objects**:
```flow
task("create_user", {
  profile: {
    name: "Alice",
    contact: {
      email: "alice@example.com",
      phone: "555-0123"
    }
  },
  settings: {
    theme: "dark",
    notifications: true
  }
})
```

**Arrays**:
```flow
task("batch_process", {
  items: [1, 2, 3],
  users: [userId1, userId2, userId3],
  mixed: [123, "string", true, null, { key: "val" }]
})
```

**Empty Structures**:
```flow
task("initialize", {
  emptyObj: {},
  emptyArr: []
})
```

### 7. Comments

```flow
// Full-line comments
await task("step1", {})  // End-of-line comments

// Multiple comment lines
// Can document complex workflows
task("step2", {})
```

### 8. Flexible Formatting

**Whitespace**:
```flow
// Compact
let x=task("t",{})

// Spaced
let   x   =   task("t", {})

// Multiline JSON
task("format", {
  key1: "value1",
  key2: "value2",
  nested: {
    inner: "value"
  }
})

// Tabs work fine
let\tx\t=\ttask("t",\t{})
```

**Windows line endings** (`\r\n`) - fully supported

**One statement per line** - Each statement must be on its own line (enforced)

## Complete Example

```flow
// E-commerce order processing workflow

// Create the order
let orderId = await task("create_order", {
  items: [
    { sku: "WIDGET-001", qty: 2, price: 19.99 },
    { sku: "GADGET-042", qty: 1, price: 49.99 }
  ],
  customer: customerId,
  total: 89_97  // Using underscores for readability
})

// Process payment
let paymentId = await task("charge_payment", {
  orderId: orderId,
  amount: 89.97,
  method: "card"
})

// Fire off analytics (don't wait)
task("track_purchase", {
  orderId: orderId,
  amount: 89.97,
  flags: 0b0001  // First-time buyer flag
})

// Generate receipt
let receiptId = await task("generate_receipt", {
  orderId: orderId,
  paymentId: paymentId
})

// Send confirmation email
await task("send_email", {
  to: customerEmail,
  template: "order-confirmation",
  data: {
    orderId: orderId,
    receiptUrl: receiptUrl
  }
})

// Mark order complete
await task("update_order_status", {
  orderId: orderId,
  status: "completed"
})
```

## What's NOT Supported

To keep things simple and focused, these are intentionally excluded:

- ❌ **Semicolons** - Unlike JavaScript, semicolons are explicitly disallowed. Each statement must be on its own line.
- ❌ **Multiple statements per line** - One statement per line only (enforced)
- ❌ **Trailing commas** - Parser currently accepts them, but not officially supported
- ❌ **Comments inside JSON** - Comments only at statement level
- ❌ **Computed keys** - Keys must be literals
- ❌ **Template strings** - Use variables instead
- ❌ **Expressions** - No arithmetic, only literal values and variable references

## Comparison with Standard JSON

| Feature | Standard JSON | Currant DSL |
|---------|--------------|-------------|
| Quoted keys | ✅ Required | ✅ Optional |
| Hex numbers | ❌ | ✅ `0xFF` |
| Binary numbers | ❌ | ✅ `0b1010` |
| Underscores in numbers | ❌ | ✅ `1_000_000` |
| Leading/trailing dots | ❌ | ✅ `.5`, `5.` |
| Comments | ❌ | ✅ `// comment` |
| Bare identifiers as values | ❌ | ✅ `{ key: varName }` |
| Single quotes | ❌ | ✅ `'string'` |
| Trailing commas | ❌ | ⚠️ Accepted but not guaranteed |

## Best Practices

1. **Use unquoted keys** for simple identifiers: `userId` instead of `"userId"`
2. **Use underscores** in large numbers for readability: `1_000_000`
3. **Use hex** for colors and addresses: `0xFF5733`
4. **Use binary** for bit flags: `0b1010`
5. **Add comments** to document complex workflows
6. **One statement per line** for clarity
7. **Use meaningful variable names**: `orderId` not `x`

## Testing

All features are thoroughly tested with 73 test cases covering:
- ✅ All number formats (hex, binary, underscores, scientific notation)
- ✅ Quoted and unquoted keys
- ✅ Variable assignment and resolution
- ✅ Escape sequences
- ✅ Unicode support
- ✅ Nested structures
- ✅ Semicolon validation (disallowed)
- ✅ Hash comment validation (disallowed, use //)
- ✅ Line validation (one statement per line)
- ✅ Edge cases (empty strings, negative numbers, deep nesting, etc.)

Run tests:
```bash
cargo test --lib interpreter
```
