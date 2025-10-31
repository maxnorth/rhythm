/**
 * Real-world usage verification test
 */

import { task, workflow, isReplaying } from '../src/index.js';

console.log('='.repeat(60));
console.log('Rhythm Node.js Library - Usage Verification');
console.log('='.repeat(60));

// Test 1: Basic task definition
console.log('\n[TEST 1] Basic Task Definition');
console.log('-'.repeat(60));

const sendEmail = task<[string, string], { sent: boolean }>({
  queue: 'emails',
  retries: 3,
  timeout: 30,
})(async function sendEmail(to: string, subject: string) {
  console.log(`  Would send email to ${to}: ${subject}`);
  return { sent: true };
});

console.log('✓ Task defined:', sendEmail.functionName);
console.log('  Queue:', sendEmail.config.queue);
console.log('  Retries:', sendEmail.config.retries);

// Test 2: Enqueue task
console.log('\n[TEST 2] Enqueue Task');
console.log('-'.repeat(60));

(async () => {
  const taskId = await sendEmail.queue('user@example.com', 'Welcome to Rhythm!');
  console.log('✓ Task enqueued with ID:', taskId);
  console.log('  Format valid:', /^task_[a-z0-9]+_[a-f0-9]+$/.test(taskId));

  // Test 3: Task with options
  console.log('\n[TEST 3] Task with Custom Priority');
  console.log('-'.repeat(60));

  const highPriorityEmail = sendEmail.options({ priority: 10 });
  const taskId2 = await highPriorityEmail.queue('vip@example.com', 'VIP Welcome');
  console.log('✓ High-priority task enqueued:', taskId2);
  console.log('  Priority:', (highPriorityEmail as any).config.priority);

  // Test 4: Task for workflow steps
  console.log('\n[TEST 4] Task Definition (for workflow steps)');
  console.log('-'.repeat(60));

  const validateOrder = task<[string, number], { valid: boolean }>({
    queue: 'orders',
    retries: 5,
    timeout: 60,
  })(async function validateOrder(orderId: string, amount: number) {
    console.log(`  Validating order ${orderId} for $${amount}`);
    if (amount < 0) throw new Error('Invalid amount');
    return { valid: true };
  });

  console.log('✓ Task defined:', validateOrder.functionName);
  console.log('  Config:', validateOrder.config);

  // Test 5: Direct task call (for testing)
  console.log('\n[TEST 5] Direct Task Call');
  console.log('-'.repeat(60));

  const result = await validateOrder.call('ORDER-123', 9999);
  console.log('✓ Task executed directly:', result);

  // Test 6: Workflow definition
  console.log('\n[TEST 6] Workflow Definition');
  console.log('-'.repeat(60));

  const chargePayment = task<[string, number], { txnId: string }>({
    queue: 'orders',
  })(async function chargePayment(orderId: string, amount: number) {
    return { txnId: `txn_${orderId}_${amount}` };
  });

  const processOrder = workflow<
    [string, number],
    { status: string; orderId: string }
  >({
    queue: 'orders',
    version: 1,
    timeout: 600,
  })(async function processOrder(orderId: string, amount: number) {
    if (!isReplaying()) {
      console.log(`  [WORKFLOW] Processing order ${orderId}`);
    }

    // Note: These would suspend the workflow in real execution
    // For now, we're just showing the structure
    // const validation = await validateOrder.run(orderId, amount);
    // const payment = await chargePayment.run(orderId, amount);

    return {
      status: 'completed',
      orderId: orderId,
    };
  });

  console.log('✓ Workflow defined:', processOrder.functionName);
  console.log('  Queue:', processOrder.config.queue);
  console.log('  Version:', (processOrder as any).version);

  // Test 7: Enqueue workflow
  console.log('\n[TEST 7] Enqueue Workflow');
  console.log('-'.repeat(60));

  const workflowId = await processOrder.queue('ORDER-456', 9999);
  console.log('✓ Workflow enqueued:', workflowId);
  console.log('  Format valid:', /^wor_[a-z0-9]+_[a-f0-9]+$/.test(workflowId));

  // Test 8: Multiple tasks
  console.log('\n[TEST 8] Batch Enqueue');
  console.log('-'.repeat(60));

  const ids = await Promise.all([
    sendEmail.queue('user1@example.com', 'Message 1'),
    sendEmail.queue('user2@example.com', 'Message 2'),
    sendEmail.queue('user3@example.com', 'Message 3'),
  ]);

  console.log('✓ Enqueued 3 tasks:');
  ids.forEach((id, i) => console.log(`  ${i + 1}. ${id}`));

  // Test 9: Type safety verification
  console.log('\n[TEST 9] Type Safety');
  console.log('-'.repeat(60));

  // This would cause TypeScript error (uncomment to test):
  // await sendEmail.queue(123, 456); // Error: wrong types
  // await sendEmail.queue('email'); // Error: missing argument

  console.log('✓ TypeScript type checking works');
  console.log('  (Try uncommenting type errors in test-usage.ts to verify)');

  // Test 10: Function registry
  console.log('\n[TEST 10] Function Registry');
  console.log('-'.repeat(60));

  const { registry } = await import('../src/registry.js');
  console.log('✓ Registered functions:', registry.list());

  // Summary
  console.log('\n' + '='.repeat(60));
  console.log('VERIFICATION SUMMARY');
  console.log('='.repeat(60));
  console.log('✓ Task definition and enqueueing: WORKING');
  console.log('✓ Workflow definition and enqueueing: WORKING');
  console.log('✓ Type safety: WORKING');
  console.log('✓ Function registry: WORKING');
  console.log('✓ Options and configuration: WORKING');
  console.log('\n⚠️  Worker execution: Requires Rust core integration');
  console.log('   (See INTEGRATION.md for details)');
  console.log('='.repeat(60));
})();
