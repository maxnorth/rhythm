# Workflow API Reference

Runtime APIs available within Rhythm workflow files (.flow).

Workflow files contain JavaScript-like code that orchestrates task execution
using async/await, loops, conditionals, and error handling.


### Table of Contents

- [Inputs](#inputs)
  - [Inputs](#inputs.inputs)
- [Task](#task)
  - [run](#task.run)
- [Time](#time)
  - [delay](#time.delay)
- [Math](#math)
  - [floor](#math.floor)
  - [ceil](#math.ceil)
  - [abs](#math.abs)
  - [round](#math.round)
- [Data Types](#data.types)
  - [string](#data.types.string)
  - [number](#data.types.number)
  - [boolean](#data.types.boolean)
  - [null](#data.types.null)
  - [array](#data.types.array)
  - [object](#data.types.object)

## Inputs

The Inputs object provides access to workflow input parameters.

Inputs are provided when queueing a workflow and are accessible
throughout the workflow execution.


### <a id="inputs.inputs"></a>Inputs `type`

```
Inputs: object
```

User-provided input data for the workflow.

Access properties using dot notation. Properties are accessed directly
from the Inputs object (e.g., `Inputs.orderId`, `Inputs.userId`).


**Examples:**

**Accessing workflow inputs**
```python
let orderId = Inputs.orderId
let amount = Inputs.amount

let result = await Task.run("process_payment", {
  orderId: orderId,
  amount: amount
})

return result

```

**Forwarding inputs to tasks**
```python
// Forward all inputs
let result = await Task.run("process_order", Inputs)

return result

```

**Nested property access**
```python
let userId = Inputs.user.id
let email = Inputs.user.email

await Task.run("send_notification", {
  userId: userId,
  email: email
})

```

## Task

The Task object provides methods for creating and executing tasks.

Tasks can be awaited for sequential execution, or run without await
for fire-and-forget behavior.


### <a id="task.run"></a>run `method`

```
Task.run(task_name: string, inputs: object): Task
```

Queue a task for execution and return a Task handle.

Use `await` to wait for the task result, or omit `await` for
fire-and-forget execution.


**Parameters:**

- **`task_name`**: Name of the task to execute (must match a @task decorated function)
- **`inputs`**: Input parameters passed to the task

**Returns:** Task handle that can be awaited for the result

**Examples:**

**Sequential execution with await**
```python
let result = await Task.run("process_payment", {
  orderId: "order-123",
  amount: 100
})

return result

```

**Fire-and-forget execution**
```python
Task.run("send_notification", {
  userId: "user-456",
  message: "Order confirmed"
})

return { success: true }

```

**Multiple sequential tasks**
```python
let payment = await Task.run("process_payment", { orderId: Inputs.orderId })
let inventory = await Task.run("update_inventory", { orderId: Inputs.orderId })
let email = await Task.run("send_email", { orderId: Inputs.orderId })

return { payment, inventory, email }

```

## Time

The Time object provides timer functionality for workflow delays.

### <a id="time.delay"></a>delay `method`

```
Time.delay(duration_ms: number): Timer
```

Create a timer that fires after the specified duration.

Use `await` to pause workflow execution until the timer fires.

**Parameters:**

- **`duration_ms`**: Duration in milliseconds

**Returns:** Timer handle that can be awaited

**Examples:**

**Simple delay**
```javascript
await Time.delay(5000)  // Wait 5 seconds
return "done"
```

**Delay between tasks**
```javascript
let result = await Task.run("process", {})
await Time.delay(10000)  // Wait 10 seconds
await Task.run("followup", { result })
```

**Capture timer for later**
```javascript
let timer = Time.delay(30000)  // Create 30s timer
let result = await Task.run("work", {})
await timer  // Wait for remaining time
return result
```

## Math

The Math object provides mathematical utility functions.


### <a id="math.floor"></a>floor `method`

```
Math.floor(x: number): number
```

Returns the largest integer less than or equal to x

**Parameters:**

- **`x`**: A numeric value

**Returns:** The floor of x

**Example:**

```python
let rounded = Math.floor(3.7)  // 3
return rounded

```

* * *

### <a id="math.ceil"></a>ceil `method`

```
Math.ceil(x: number): number
```

Returns the smallest integer greater than or equal to x

**Parameters:**

- **`x`**: A numeric value

**Returns:** The ceiling of x

**Example:**

```python
let rounded = Math.ceil(3.2)  // 4
return rounded

```

* * *

### <a id="math.abs"></a>abs `method`

```
Math.abs(x: number): number
```

Returns the absolute value of x

**Parameters:**

- **`x`**: A numeric value

**Returns:** The absolute value of x

**Example:**

```python
let positive = Math.abs(-5)  // 5
return positive

```

* * *

### <a id="math.round"></a>round `method`

```
Math.round(x: number): number
```

Returns x rounded to the nearest integer.

Uses JavaScript-style rounding where half-way cases round towards +∞
(e.g., 2.5 → 3, -2.5 → -2).


**Parameters:**

- **`x`**: A numeric value

**Returns:** The rounded value of x

**Example:**

```python
let rounded = Math.round(3.5)  // 4
return rounded

```

## Data Types

Workflows support standard JSON data types.


### <a id="data.types.string"></a>string `type`

```python
stringstring
```

Text values enclosed in double quotes

**Example:**

```python
"hello world"
"user@example.com"

```

* * *

### <a id="data.types.number"></a>number `type`

```python
numbernumber
```

Numeric values (integers or floating-point)

**Example:**

```python
42
3.14159
-17

```

* * *

### <a id="data.types.boolean"></a>boolean `type`

```python
booleanboolean
```

Logical true or false values

**Example:**

```python
true
false

```

* * *

### <a id="data.types.null"></a>null `type`

```python
nullnull
```

Represents the absence of a value

**Example:**

```python
null

```

* * *

### <a id="data.types.array"></a>array `type`

```python
arrayarray
```

Ordered lists of values enclosed in square brackets

**Example:**

```python
[1, 2, 3]
["a", "b", "c"]
[1, "mixed", true, null]

```

* * *

### <a id="data.types.object"></a>object `type`

```python
objectobject
```

Key-value maps enclosed in curly braces.

Keys can be unquoted identifiers or quoted strings. Use shorthand
syntax when the key matches a variable name.


**Example:**

```python
// Quoted keys
{ "name": "Alice", "age": 30 }

// Unquoted keys
{ name: "Alice", age: 30 }

// Shorthand syntax
let orderId = "order-123"
{ orderId, status: "pending" }  // Same as { orderId: orderId, status: "pending" }

```
