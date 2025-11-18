/**
 * Rhythm - A lightweight durable execution framework using only Postgres
 */

export { task } from './decorators.js';
export { getExecutionStatus, cancelExecution } from './client.js';
export { Worker, runWorker, type WorkerOptions } from './worker.js';
export { RustBridge } from './rust-bridge-native.js';

export type {
  ExecutableProxy,
  TaskConfig,
  ExecutionConfig,
  ExecutionStatus,
} from './types.js';

export const version = '0.1.0';
