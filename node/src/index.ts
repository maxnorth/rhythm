/**
 * Currant - A lightweight durable execution framework using only Postgres
 */

export { task, workflow } from './decorators.js';
export { sendSignal, getExecutionStatus, cancelExecution } from './client.js';
export { waitForSignal, getVersion, isReplaying } from './context.js';
export { Worker, runWorker, type WorkerOptions } from './worker.js';
export { RustBridge } from './rust-bridge-native.js';

export type {
  ExecutableProxy,
  TaskConfig,
  WorkflowConfig,
  ExecutionConfig,
  SignalPayload,
  ExecutionStatus,
} from './types.js';

export const version = '0.1.0';
