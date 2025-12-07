# Python API Reference

Complete API reference for the Rhythm Python SDK

### Table of Contents

- [Initialization](#initialization)
  - [init](#init)
- [Tasks](#tasks)
  - [task](#task)
- [Client](#client)
  - [cancel_execution](#cancel_execution)
  - [get_execution](#get_execution)
  - [list_executions](#list_executions)
  - [queue_execution](#queue_execution)
  - [queue_task](#queue_task)
  - [queue_workflow](#queue_workflow)
  - [start_workflow](#start_workflow)
  - [wait_for_execution](#wait_for_execution)
- [Worker](#worker)
  - [run](#run)

## Initialization

Initialize Rhythm and configure your application for task execution.

The initialization process connects to your PostgreSQL database, scans for workflow
definitions, and prepares the system for executing tasks and workflows.

**Basic initialization**
Initialize with database connection

```python
import rhythm

rhythm.init("postgresql://rhythm@localhost/rhythm")

```

**Custom workflow paths**
Specify custom directories containing .flow files

```python
import rhythm

rhythm.init(
    "postgresql://rhythm@localhost/rhythm",
    workflow_paths=["./workflows", "./custom-flows"]
)

```

### init `function`

```python
init(database_url: str, workflow_paths: Optional[List[str]] = None, auto_migrate: bool = True) -> None
```

Initialize Rhythm with workflow definitions.

This function initializes the Rust core with a database connection,
scans for .flow workflow files, and prepares the system for execution.

**Parameters:**

- **`database_url`**: PostgreSQL connection string
- **`workflow_paths`**: List of paths to directories containing .flow files
- **`auto_migrate`**: Whether to automatically run migrations if needed

## Tasks

Define and execute background tasks using the @task decorator.

Tasks can be called synchronously or queued for asynchronous execution. The decorator
adds a `.queue()` method to your functions for background processing.

### task `decorator`

```python
task(fn: Optional[Callable] = None, *, queue: str = 'default')
```

Mark a function as a Rhythm task that can be queued for async execution.

Decorated functions can be called directly (synchronous) or queued for
async execution via the added `.queue()` method.

**Parameters:**

- **`queue`**: The queue name to execute in (defaults to "default")

**Returns:** The decorated function with an added `.queue()` method

**Example:**

```python
@task
    def send_email(to: str, subject: str):
        email_client.send(to, subject)

    # Direct call (synchronous)
    send_email("user@example.com", "Hello")

    # Queue for async execution
    execution_id = send_email.queue(to="user@example.com", subject="Hello")
```

## Client

Client functions for queuing tasks, managing executions, and checking execution status.

Use these functions to interact with the Rhythm execution system from your application code.

### cancel_execution `function`

```python
cancel_execution(execution_id: str) -> bool
```

Cancel a pending or suspended execution.

**Parameters:**

- **`execution_id`**: The execution ID

**Returns:** True if cancelled, False if not found or already completed/running

### get_execution `function`

```python
get_execution(execution_id: str) -> Optional[rhythm.models.Execution]
```

Get an execution by ID.

**Parameters:**

- **`execution_id`**: The execution ID

**Returns:** Execution object or None if not found

### list_executions `function`

```python
list_executions(queue: Optional[str] = None, status: Optional[str] = None, limit: int = 100, offset: int = 0) -> list[dict]
```

List executions with optional filters.

NOTE: This function is currently not implemented as it requires direct database access.
Use the Rust bridge functions instead for execution management.

**Parameters:**

- **`queue`**: Filter by queue name
- **`status`**: Filter by status
- **`limit`**: Maximum number of results
- **`offset`**: Offset for pagination

**Returns:** List of execution dicts

### queue_execution `function`

```python
queue_execution(exec_type: str, target_name: str, inputs: dict, queue: str, parent_workflow_id: Optional[str] = None) -> str
```

Enqueue an execution (task or workflow).

Note: Prefer using queue_task() or queue_workflow() for better type safety.

**Parameters:**

- **`exec_type`**: Type of execution ('task', 'workflow')
- **`target_name`**: Target name (task or workflow name)
- **`inputs`**: Input parameters as a dictionary
- **`queue`**: Queue name
- **`parent_workflow_id`**: Parent workflow ID (for workflow tasks)

**Returns:** Execution ID

### queue_task `function`

```python
queue_task(name: str, inputs: dict, queue: str = 'default') -> str
```

Queue a task for execution.

**Parameters:**

- **`name`**: Task function name
- **`inputs`**: Input parameters as a dictionary
- **`queue`**: Queue name (default: "default")

**Returns:** Execution ID

### queue_workflow `function`

```python
queue_workflow(name: str, inputs: dict, queue: str = 'default') -> str
```

Queue a workflow for execution.

**Parameters:**

- **`name`**: Workflow name
- **`inputs`**: Input parameters as a dictionary
- **`queue`**: Queue name (default: "default")

**Returns:** Execution ID

### start_workflow `function`

```python
start_workflow(workflow_name: str, inputs: dict[str, typing.Any]) -> str
```

Start a workflow execution.

**Parameters:**

- **`workflow_name`**: Name of the workflow to execute (matches .flow filename)
- **`inputs`**: Input parameters for the workflow

**Returns:** Workflow execution ID

**Example:**

```python
workflow_id = rhythm.start_workflow(
        "processOrder",
        inputs={"orderId": "order-123", "amount": 99.99}
    )
```

### wait_for_execution `function`

```python
wait_for_execution(execution_id: str, timeout: float = 60.0, poll_interval: float = 0.5) -> rhythm.models.Execution
```

Wait for an execution to reach a terminal state and return it.

Polls the execution status until it reaches "completed" or "failed" status.

**Parameters:**

- **`execution_id`**: The execution ID to wait for
- **`timeout`**: Maximum time to wait in seconds (default: 60)
- **`poll_interval`**: How often to poll in seconds (default: 0.5)

**Returns:** Execution object with full execution details

**Raises:**

- `TimeoutError: If execution doesn't reach terminal state within timeout`
- `RuntimeError: If execution not found`

## Worker

Worker functions for processing queued tasks and workflows.

Workers poll the database for pending executions and process them sequentially.

### run `function`

```python
run()
```

Run a worker loop that polls for and executes tasks.

The worker runs synchronously in a single thread, polling the database
for pending tasks and executing them one at a time.
