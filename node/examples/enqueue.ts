/**
 * Enqueue example tasks from imported module
 */

import { sendNotification, processOrderWorkflow } from './simple.js';

async function main() {
  console.log('='.repeat(60));
  console.log('Enqueuing example tasks and workflows');
  console.log('='.repeat(60) + '\n');

  // Enqueue a simple notification task
  const taskId = await sendNotification.queue('user_123', 'Your order has been confirmed!');
  console.log(`✓ Notification task enqueued: ${taskId}\n`);

  // Enqueue an order processing workflow
  const workflowId = await processOrderWorkflow.queue(
    'order_456',
    'customer@example.com',
    9999,
    'credit_card',
    ['item1', 'item2', 'item3']
  );
  console.log(`✓ Order workflow enqueued: ${workflowId}\n`);

  // Enqueue another order
  const workflowId2 = await processOrderWorkflow.queue(
    'order_789',
    'vip@example.com',
    19999,
    'credit_card',
    ['premium_item']
  );
  console.log(`✓ VIP order workflow enqueued: ${workflowId2}\n`);

  console.log('='.repeat(60));
  console.log('Tasks and workflows enqueued! Start workers to process them:');
  console.log('  rhythm worker -q notifications -q orders');
  console.log('='.repeat(60));
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}
