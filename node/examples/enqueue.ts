/**
 * Enqueue example jobs from imported module
 */

import { sendNotification, processOrderWorkflow } from './simple.js';

async function main() {
  console.log('='.repeat(60));
  console.log('Enqueuing example jobs and workflows');
  console.log('='.repeat(60) + '\n');

  // Enqueue a simple notification job
  const jobId = await sendNotification.queue('user_123', 'Your order has been confirmed!');
  console.log(`✓ Notification job enqueued: ${jobId}\n`);

  // Enqueue an order processing workflow
  const workflowId = await processOrderWorkflow.queue(
    'order_456',
    'customer@example.com',
    9999,
    'credit_card',
    ['item1', 'item2', 'item3']
  );
  console.log(`✓ Order workflow enqueued: ${workflowId}\n`);

  // Enqueue another order with high priority
  const workflowId2 = await processOrderWorkflow
    .options({ priority: 10 })
    .queue('order_789', 'vip@example.com', 19999, 'credit_card', ['premium_item']);
  console.log(`✓ High-priority order workflow enqueued: ${workflowId2}\n`);

  console.log('='.repeat(60));
  console.log('Jobs enqueued! Start workers to process them:');
  console.log('  currant worker -q notifications -q orders');
  console.log('='.repeat(60));
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}
