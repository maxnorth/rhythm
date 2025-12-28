# Signals Example

This example demonstrates human-in-the-loop workflows using signals.

## Overview

Signals enable workflows to pause and wait for external input. This is useful for:

- **Approval workflows**: Wait for a manager to approve a request
- **Human review**: Pause for manual verification before proceeding
- **External events**: Wait for webhooks, user actions, or other external triggers

## How it works

1. A workflow calls `Signal.next("channel_name")` to wait for a signal
2. The workflow suspends until a signal arrives
3. An external system sends a signal via `rhythm.client.send_signal()`
4. The workflow resumes with the signal's payload

## Files

- `workflows/approval.flow` - Approval workflow that waits for a signal
- `tasks.py` - Task implementations (validation, payment, notifications)
- `worker.py` - Worker process that executes workflows and tasks
- `app.py` - Starts a workflow and waits for completion
- `send_approval.py` - CLI tool to send approval/rejection signals

## Running the example

### Terminal 1: Start the worker

```bash
cd python/examples/signals
python worker.py
```

### Terminal 2: Start a workflow

```bash
cd python/examples/signals
python app.py
```

This will output something like:
```
Workflow started: 550e8400-e29b-41d4-a716-446655440000

The workflow is now waiting for an approval signal.

To APPROVE, run:
    python send_approval.py 550e8400-e29b-41d4-a716-446655440000

To REJECT, run:
    python send_approval.py 550e8400-e29b-41d4-a716-446655440000 --reject --reason "Your reason"
```

### Terminal 3: Send the approval signal

```bash
cd python/examples/signals

# Approve the workflow
python send_approval.py 550e8400-e29b-41d4-a716-446655440000

# Or reject it
python send_approval.py 550e8400-e29b-41d4-a716-446655440000 --reject --reason "Budget exceeded"
```

After sending the signal, the workflow in Terminal 2 will complete.

## Command-line options

### app.py

```bash
python app.py --order-id "order-123" --amount 500.00 --customer-id "cust-456"
```

### send_approval.py

```bash
# Approve
python send_approval.py <workflow_id>

# Reject with reason
python send_approval.py <workflow_id> --reject --reason "Not approved"

# Specify reviewer
python send_approval.py <workflow_id> --reviewer "alice@example.com"
```

## Key concepts

### Workflow: Waiting for a signal

```javascript
// Wait for approval signal - workflow suspends here
let approval = await Signal.next("approval")

// approval contains the payload sent via send_signal()
if (approval.approved) {
    // proceed with order
}
```

### Client: Sending a signal

```python
import rhythm

# Send approval to a waiting workflow
rhythm.client.send_signal(
    workflow_id="abc-123",
    signal_name="approval",
    payload={
        "approved": True,
        "reviewer": "alice@example.com"
    }
)
```

## Signal semantics

- Signals are **matched by name** within a workflow
- If multiple signals are sent before the workflow reaches `Signal.next()`, they queue up (FIFO)
- If a signal arrives before the workflow reaches `Signal.next()`, it's stored and matched on resumption
- Each `Signal.next()` call consumes one signal from the queue
