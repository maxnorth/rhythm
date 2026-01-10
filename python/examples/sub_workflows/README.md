# Sub-Workflows Example

This example demonstrates how to use **sub-workflows** (child workflows) in Rhythm. A parent workflow can spawn child workflows using `Workflow.run()`, wait for their results, and use those results to make decisions.

## Overview

The example implements an order fulfillment system with:

- **Parent Workflow: `order_fulfillment`** - Orchestrates the entire order process
- **Child Workflow: `process_payment`** - Handles payment validation and charging
- **Child Workflow: `reserve_inventory`** - Checks and reserves inventory
- **Child Workflow: `arrange_shipping`** - Sets up delivery and tracking

## How It Works

```
order_fulfillment (parent)
    │
    ├── Workflow.run("process_payment", {...})
    │   └── Tasks: validate_payment_method, charge_payment, record_transaction
    │
    ├── Workflow.run("reserve_inventory", {...})
    │   └── Tasks: check_inventory, reserve_item, release_reservation
    │
    └── Workflow.run("arrange_shipping", {...})
        └── Tasks: get_customer_address, calculate_shipping, create_shipment, schedule_pickup
```

## Running the Example

1. **Start the worker** (in one terminal):
   ```bash
   cd python/examples/sub_workflows
   python worker.py
   ```

2. **Run the app** (in another terminal):
   ```bash
   cd python/examples/sub_workflows
   python app.py
   ```

## Key Concepts

### Spawning a Child Workflow

```javascript
// In parent workflow
let result = await Workflow.run("child_workflow_name", {
    input1: value1,
    input2: value2
})
```

### Handling Child Results

Child workflows return values just like tasks:

```javascript
let payment = await Workflow.run("process_payment", { ... })

if (!payment.success) {
    // Handle payment failure
    return { status: "failed" }
}

// Continue with payment.transaction_id
```

### Fire-and-Forget Child Workflows

You can also spawn child workflows without waiting:

```javascript
// Start child workflow but don't wait for it
Workflow.run("background_process", { ... })

// Continue immediately
```

## Files

- `workflows/order_fulfillment.flow` - Parent workflow
- `workflows/process_payment.flow` - Payment child workflow
- `workflows/reserve_inventory.flow` - Inventory child workflow
- `workflows/arrange_shipping.flow` - Shipping child workflow
- `tasks.py` - Task implementations
- `worker.py` - Worker entry point
- `app.py` - Demo application
