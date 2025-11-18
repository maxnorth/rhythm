/**
 * Client API for enqueuing work
 */

import { RustBridge } from './rust-bridge-native.js';
import type { ExecutionStatus } from './types.js';

export async function queueExecution(params: {
  execType: string;
  functionName: string;
  args: any[];
  kwargs?: Record<string, any>;
  queue: string;
  priority?: number;
  maxRetries?: number;
  timeoutSeconds?: number;
  parentWorkflowId?: string;
}): Promise<string> {
  const executionId = RustBridge.createExecution({
    execType: params.execType,
    functionName: params.functionName,
    queue: params.queue,
    priority: params.priority ?? 5,
    args: params.args,
    kwargs: params.kwargs || {},
    maxRetries: params.maxRetries ?? 3,
    timeoutSeconds: params.timeoutSeconds,
    parentWorkflowId: params.parentWorkflowId,
  });

  console.info(
    `Enqueued ${params.execType} ${executionId}: ${params.functionName} on queue ${params.queue}`
  );

  return executionId;
}

export async function getExecutionStatus(executionId: string): Promise<ExecutionStatus | null> {
  return RustBridge.getExecution(executionId);
}

export async function cancelExecution(executionId: string): Promise<boolean> {
  try {
    RustBridge.failExecution(
      executionId,
      { message: 'Execution cancelled', type: 'CancellationError' },
      false
    );
    console.info(`Execution ${executionId} cancelled`);
    return true;
  } catch (error) {
    console.warn(`Could not cancel execution ${executionId}:`, error);
    return false;
  }
}
