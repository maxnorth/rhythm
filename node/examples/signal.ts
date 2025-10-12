/**
 * Example demonstrating workflow signals for human-in-the-loop workflows
 */

import { workflow, task, waitForSignal } from '../src/index.js';

const prepareDocument = task<[string], { prepared: boolean; doc_id: string }>({
  queue: 'documents',
})(async (docId: string) => {
  console.log(`[PREPARE] Preparing document ${docId} for review`);
  await new Promise((resolve) => setTimeout(resolve, 500));
  return { prepared: true, doc_id: docId };
});

const publishDocument = task<[string], { published: boolean; doc_id: string; url: string }>({
  queue: 'documents',
})(async (docId: string) => {
  console.log(`[PUBLISH] Publishing document ${docId}`);
  await new Promise((resolve) => setTimeout(resolve, 300));
  return {
    published: true,
    doc_id: docId,
    url: `https://example.com/docs/${docId}`,
  };
});

const archiveDocument = task<[string], { archived: boolean; doc_id: string }>({
  queue: 'documents',
})(async (docId: string) => {
  console.log(`[ARCHIVE] Archiving rejected document ${docId}`);
  await new Promise((resolve) => setTimeout(resolve, 200));
  return { archived: true, doc_id: docId };
});

type ApprovalResult =
  | {
      status: 'approved';
      doc_id: string;
      url: string;
      approved_by?: string;
    }
  | {
      status: 'rejected';
      doc_id: string;
      rejected_by?: string;
      reason?: string;
    };

const documentApprovalWorkflow = workflow<[string, string], ApprovalResult>({
  queue: 'documents',
  version: 1,
  timeout: 86400, // 24 hour timeout
})(async (docId: string, author: string) => {
  /**
   * Document approval workflow that waits for human approval.
   *
   * The workflow will suspend and wait for an external signal.
   */
  console.log(`\n[WORKFLOW] Starting approval process for document ${docId} by ${author}\n`);

  // Prepare the document
  const prepResult = await prepareDocument.run(docId);
  console.log(`[WORKFLOW] ✓ Document prepared: ${JSON.stringify(prepResult)}\n`);

  // Wait for approval signal (workflow suspends here)
  console.log(`[WORKFLOW] ⏸  Waiting for approval signal...\n`);
  const approval = await waitForSignal('approval_decision', 86400); // 24 hours
  console.log(`[WORKFLOW] ▶  Received approval signal: ${JSON.stringify(approval)}\n`);

  // Process based on approval
  if (approval.approved) {
    const publishResult = await publishDocument.run(docId);
    console.log(`[WORKFLOW] ✓ Document published: ${publishResult.url}\n`);

    return {
      status: 'approved',
      doc_id: docId,
      url: publishResult.url,
      approved_by: approval.approved_by,
    };
  } else {
    await archiveDocument.run(docId);
    console.log(`[WORKFLOW] ✓ Document archived\n`);

    return {
      status: 'rejected',
      doc_id: docId,
      rejected_by: approval.approved_by,
      reason: approval.reason,
    };
  }
});

async function main() {
  console.log('='.repeat(60));
  console.log('Document Approval Workflow Example');
  console.log('='.repeat(60) + '\n');

  // Start the approval workflow
  const workflowId = await documentApprovalWorkflow.queue('doc_12345', 'john@example.com');
  console.log(`✓ Approval workflow started: ${workflowId}\n`);

  console.log('='.repeat(60));
  console.log('Workflow is now waiting for approval signal.');
  console.log('Start a worker and then send a signal:');
  console.log('  currant worker -q documents\n');
  console.log('To approve:');
  console.log(
    `  node -e "import('./src/index.js').then(m => m.sendSignal('${workflowId}', 'approval_decision', {approved: true, approved_by: 'manager@example.com'}))"`
  );
  console.log('\nTo reject:');
  console.log(
    `  node -e "import('./src/index.js').then(m => m.sendSignal('${workflowId}', 'approval_decision', {approved: false, approved_by: 'manager@example.com', reason: 'Needs revision'}))"`
  );
  console.log('='.repeat(60));
}

// Run if this is the main module
if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}

export { documentApprovalWorkflow };
