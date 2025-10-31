# Rhythm - Node.js Library

A lightweight durable execution framework using only Postgres. This is the Node.js/TypeScript implementation.

> **Note on Worker Execution**: The Node.js library is fully functional for defining and enqueueing work. Worker execution currently requires the Python CLI or future Rust FFI integration. See [INTEGRATION.md](INTEGRATION.md) for details.

## Installation

### Quick Start (TypeScript only)

```bash
npm install
npm run build
npm test
```

This works without Rust for development and testing (uses stub mode).

### Full Installation (With Native Bindings)

```bash
# 1. Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Build native bindings
cd native && npm install && npm run build && cd ..

# 3. Install and build
npm install
npm run build

# 4. Setup database
export RHYTHM_DATABASE_URL="postgresql://localhost/rhythm"
node dist/cli.js migrate
```

See [BUILD.md](BUILD.md) for detailed instructions.

## Quick Start

```typescript
import { task, workflow, isReplaying } from 'rhythm';

// Define a simple task
const sendEmail = task<[string, string], void>({
  queue: 'emails',
  retries: 3
})(async (to: string, subject: string) => {
  console.log(`Sending email to ${to}: ${subject}`);
  // Email sending logic here
});

// Define tasks for workflow steps
const chargeCard = task<[number, string], { transactionId: string }>({
  retries: 5,
  timeout: 120
})(async (amount: number, cardToken: string) => {
  // Payment processing logic
  return { transactionId: 'txn_123' };
});

const sendReceipt = task<[string, number], void>()(
  async (email: string, amount: number) => {
    // Receipt sending logic
  }
);

// Define a workflow that orchestrates tasks
const processPayment = workflow<[string, number, string], { status: string }>({
  queue: 'payments',
  version: 1,
  timeout: 600
})(async (email: string, amount: number, cardToken: string) => {
  if (!isReplaying()) {
    console.log('Starting payment processing...');
  }

  // Tasks are checkpointed and can be replayed
  const payment = await chargeCard.run(amount, cardToken);

  if (!isReplaying()) {
    console.log(`Payment successful: ${payment.transactionId}`);
  }

  await sendReceipt.run(email, amount);

  return { status: 'completed' };
});

// Enqueue work
await sendEmail.queue('user@example.com', 'Welcome!');
await processPayment.queue('user@example.com', 9999, 'tok_visa');
```

## Core Concepts

### Tasks

Tasks are standalone async units of work that run independently:

```typescript
const processImage = task<[string], { url: string }>({
  queue: 'images',
  retries: 3,
  timeout: 300,
  priority: 5
})(async (imageId: string) => {
  // Process image
  return { url: 'https://...' };
});

// Enqueue the task
const taskId = await processImage.queue('img_123');
```

### Workflow Steps

Tasks can also be workflow steps that are automatically checkpointed:

```typescript
const validateOrder = task<[string, number], { valid: boolean }>({
  retries: 3,
  timeout: 60
})(async (orderId: string, amount: number) => {
  // Validation logic
  return { valid: true };
});

// Tasks must be called from within a workflow using .run()
const result = await validateOrder.run('order_123', 9999);
```

### Workflows

Workflows orchestrate multiple tasks with automatic retry and recovery:

```typescript
const processOrder = workflow<
  [string, number],
  { status: string }
>({
  queue: 'orders',
  version: 1,
  timeout: 600
})(async (orderId: string, amount: number) => {
  // Each task is checkpointed
  const validation = await validateOrder.run(orderId, amount);
  const payment = await chargePayment.run(orderId, amount);
  const shipment = await shipOrder.run(orderId);

  return { status: 'completed' };
});
```

### Signals

Workflows can wait for external signals (human-in-the-loop):

```typescript
import { waitForSignal, sendSignal } from 'rhythm';

const approvalWorkflow = workflow<[string], { approved: boolean }>({
  queue: 'approvals',
  timeout: 86400 // 24 hours
})(async (documentId: string) => {
  await prepareDocument.run(documentId);

  // Workflow suspends here until signal arrives
  const decision = await waitForSignal('approval_decision', 86400);

  if (decision.approved) {
    await publishDocument.run(documentId);
  }

  return { approved: decision.approved };
});

// Send a signal to resume the workflow
await sendSignal(workflowId, 'approval_decision', {
  approved: true,
  approver: 'manager@example.com'
});
```

### Workflow Versioning

Evolve workflows while maintaining backward compatibility:

```typescript
import { getVersion } from 'rhythm';

const processOrder = workflow<[string], any>({
  queue: 'orders',
  version: 2
})(async (orderId: string) => {
  const payment = await chargeCard.run(orderId);

  // New feature added in version 2
  if (getVersion('send_sms', 1, 2) >= 2) {
    await sendSMS.run(orderId);
  }

  await sendEmail.run(orderId);
});
```

## API Reference

### Decorators

- `task<TArgs, TReturn>(config)` - Define a task (standalone or workflow step)
- `workflow<TArgs, TReturn>(config)` - Define a workflow

### Client Functions

- `sendSignal(workflowId, signalName, payload?)` - Send a signal to a workflow
- `getExecutionStatus(executionId)` - Get execution status
- `cancelExecution(executionId)` - Cancel a pending/suspended execution

### Context Functions (use within workflows)

- `isReplaying()` - Check if currently replaying from history
- `waitForSignal(signalName, timeout?)` - Wait for an external signal
- `getVersion(changeId, minVersion, maxVersion)` - Get version for workflow evolution

## Examples

See the `examples/` directory for complete examples:

- `examples/simple.ts` - Tasks and workflows
- `examples/signal.ts` - Human-in-the-loop with signals
- `examples/enqueue.ts` - Enqueuing from imported modules

Run examples:

```bash
npm run example:simple
npm run example:signal
npm run example:enqueue
```

## Development

```bash
# Install dependencies
npm install

# Build
npm run build

# Run tests
npm test

# Run tests in watch mode
npm run test:watch

# Lint
npm run lint

# Format
npm run format
```

## TypeScript Support

This library is written in TypeScript and includes full type definitions. All decorators support generic type parameters for type-safe arguments and return values.

## License

MIT
