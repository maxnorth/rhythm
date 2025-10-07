/**
 * Simple example demonstrating jobs, activities, and workflows
 */

import { job, activity, workflow, isReplaying } from '../src/index.js';

// Simple job that runs independently
const sendNotification = job<[string, string], { sent: boolean; user_id: string }>({
  name: 'sendNotification',
  queue: 'notifications',
  retries: 3,
})(async (userId: string, message: string) => {
  console.log(`[NOTIFICATION] Sending to user ${userId}: ${message}`);
  await new Promise((resolve) => setTimeout(resolve, 500)); // Simulate API call
  return { sent: true, user_id: userId };
});

// Activities that are called from workflows
const validateOrder = activity<[string, number], { valid: boolean; order_id: string }>({
  name: 'validateOrder',
  retries: 3,
  timeout: 60,
})(async (orderId: string, amount: number) => {
  console.log(`[VALIDATE] Validating order ${orderId} for $${amount}`);
  await new Promise((resolve) => setTimeout(resolve, 300));

  if (amount < 0) {
    throw new Error('Amount must be positive');
  }

  return { valid: true, order_id: orderId };
});

const chargePayment = activity<
  [string, number, string],
  { success: boolean; transaction_id: string; amount: number }
>({
  name: 'chargePayment',
  retries: 5,
  timeout: 120,
})(async (orderId: string, amount: number, paymentMethod: string) => {
  console.log(`[CHARGE] Charging $${amount} via ${paymentMethod} for order ${orderId}`);
  await new Promise((resolve) => setTimeout(resolve, 500));

  // Simulate payment processing
  const transactionId = `txn_${orderId}_${paymentMethod}`;

  return {
    success: true,
    transaction_id: transactionId,
    amount: amount,
  };
});

const sendConfirmationEmail = activity<[string, string, number], { sent: boolean; email: string }>(
  { name: 'sendConfirmationEmail' }
)(async (email: string, orderId: string, amount: number) => {
  console.log(`[EMAIL] Sending confirmation to ${email} for order ${orderId} ($${amount})`);
  await new Promise((resolve) => setTimeout(resolve, 200));
  return { sent: true, email: email };
});

const updateInventory = activity<[string, string[]], { updated: boolean; item_count: number }>({
  name: 'updateInventory',
})(async (orderId: string, items: string[]) => {
    console.log(`[INVENTORY] Updating inventory for order ${orderId}: ${items.length} items`);
    await new Promise((resolve) => setTimeout(resolve, 300));
    return { updated: true, item_count: items.length };
  }
);

// Workflow that orchestrates the order processing
const processOrderWorkflow = workflow<
  [string, string, number, string, string[]],
  {
    status: string;
    order_id: string;
    transaction_id: string;
    amount: number;
  }
>({
  name: 'processOrderWorkflow',
  queue: 'orders',
  version: 1,
  timeout: 600,
})(
  async (
    orderId: string,
    customerEmail: string,
    amount: number,
    paymentMethod: string,
    items: string[]
  ) => {
    /**
     * Process an order end-to-end with automatic retry and recovery.
     *
     * This workflow will survive crashes and resume from checkpoints.
     */
    if (!isReplaying()) {
      console.log(`\n[WORKFLOW] Starting order processing for ${orderId}\n`);
    }

    // Step 1: Validate the order
    const validationResult = await validateOrder.run(orderId, amount);
    if (!isReplaying()) {
      console.log(`[WORKFLOW] ✓ Validation completed: ${JSON.stringify(validationResult)}\n`);
    }

    // Step 2: Charge the payment
    const paymentResult = await chargePayment.run(orderId, amount, paymentMethod);
    if (!isReplaying()) {
      console.log(`[WORKFLOW] ✓ Payment charged: ${paymentResult.transaction_id}\n`);
    }

    // Step 3: Send confirmation email
    const emailResult = await sendConfirmationEmail.run(customerEmail, orderId, amount);
    if (!isReplaying()) {
      console.log(`[WORKFLOW] ✓ Email sent: ${JSON.stringify(emailResult)}\n`);
    }

    // Step 4: Update inventory
    const inventoryResult = await updateInventory.run(orderId, items);
    if (!isReplaying()) {
      console.log(`[WORKFLOW] ✓ Inventory updated: ${JSON.stringify(inventoryResult)}\n`);
    }

    return {
      status: 'completed',
      order_id: orderId,
      transaction_id: paymentResult.transaction_id,
      amount: amount,
    };
  }
);

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
  console.log('  currant worker -q notifications');
  console.log('  currant worker -q orders');
  console.log('='.repeat(60));
}

// Run if this is the main module
if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}

export { sendNotification, processOrderWorkflow };
