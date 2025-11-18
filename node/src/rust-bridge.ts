/**
 * Bridge to Rust core - Node.js implementation
 * This should be replaced with actual FFI bindings when available
 */

import { generateId } from './utils.js';

export interface CreateExecutionParams {
  execType: string;
  functionName: string;
  queue: string;
  priority: number;
  args: any[];
  kwargs: Record<string, any>;
  maxRetries: number;
  timeoutSeconds?: number;
  parentWorkflowId?: string;
}

class RustBridgeImpl {
  createExecution(params: CreateExecutionParams): string {
    // TODO: Replace with actual Rust FFI call
    const executionId = generateId(params.execType.substring(0, 3));

    // For now, we'll need to implement this via CLI or direct DB access
    // eslint-disable-next-line no-console
    console.warn('RustBridge.createExecution is a stub - needs Rust FFI implementation');
    // Prevent unused variable warnings
    void params;

    return executionId;
  }

  getExecution(executionId: string): any {
    // TODO: Replace with actual Rust FFI call
    // eslint-disable-next-line no-console
    console.warn('RustBridge.getExecution is a stub - needs Rust FFI implementation');
    // Prevent unused variable warnings
    void executionId;

    return null;
  }

  failExecution(executionId: string, error: any, retry: boolean): void {
    // TODO: Replace with actual Rust FFI call
    // eslint-disable-next-line no-console
    console.warn('RustBridge.failExecution is a stub - needs Rust FFI implementation');
    // Prevent unused variable warnings
    void executionId;
    void error;
    void retry;
  }
}

export const RustBridge = new RustBridgeImpl();
