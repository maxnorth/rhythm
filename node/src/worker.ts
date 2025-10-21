/**
 * Worker implementation for executing tasks and workflows
 */

import { RustBridge } from './rust-bridge-native.js';
import { registry } from './registry.js';
import { generateId } from './utils.js';
import { settings } from './config.js';

export interface WorkerOptions {
  queues: string[];
  workerId?: string;
  maxConcurrent?: number;
  heartbeatInterval?: number;
  pollInterval?: number;
}

export class Worker {
  private workerId: string;
  private queues: string[];
  private running: boolean = false;
  private currentExecutions: number = 0;
  private maxConcurrent: number;
  private heartbeatInterval: number;
  private pollInterval: number;

  private heartbeatTimer?: NodeJS.Timeout;
  private pollTimer?: NodeJS.Timeout;
  private recoveryTimer?: NodeJS.Timeout;

  constructor(options: WorkerOptions) {
    this.workerId = options.workerId || generateId('worker');
    this.queues = options.queues;
    this.maxConcurrent = options.maxConcurrent || 10;
    this.heartbeatInterval = options.heartbeatInterval || 5000;
    this.pollInterval = options.pollInterval || 1000;

    console.log(`Worker ${this.workerId} initialized for queues: ${this.queues.join(', ')}`);
  }

  async start(): Promise<void> {
    if (!RustBridge.isAvailable()) {
      throw new Error(
        'Native bindings not available. Build with: cd ../node-bindings && npm run build'
      );
    }

    this.running = true;
    console.log(`Worker ${this.workerId} starting...`);

    // Setup signal handlers
    this.setupSignalHandlers();

    // Start background tasks
    this.startHeartbeat();
    this.startPolling();
    this.startRecovery();

    console.log(`Worker ${this.workerId} running`);
  }

  async stop(): Promise<void> {
    console.log(`Worker ${this.workerId} stopping...`);
    this.running = false;

    // Clear timers
    if (this.heartbeatTimer) clearInterval(this.heartbeatTimer);
    if (this.pollTimer) clearInterval(this.pollTimer);
    if (this.recoveryTimer) clearInterval(this.recoveryTimer);

    // Update worker status
    await RustBridge.stopWorker(this.workerId);

    // Wait for current executions to complete (with timeout)
    const timeout = 30000;
    const start = Date.now();
    while (this.currentExecutions > 0) {
      if (Date.now() - start > timeout) {
        console.warn('Timeout waiting for executions to complete');
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, 500));
    }

    console.log(`Worker ${this.workerId} stopped`);
  }

  private setupSignalHandlers(): void {
    const shutdown = () => {
      console.log('Received shutdown signal');
      this.stop().then(() => process.exit(0));
    };

    process.on('SIGINT', shutdown);
    process.on('SIGTERM', shutdown);
  }

  private startHeartbeat(): void {
    this.heartbeatTimer = setInterval(async () => {
      try {
        await RustBridge.updateHeartbeat(this.workerId, this.queues);
      } catch (error) {
        console.error('Error updating heartbeat:', error);
      }
    }, this.heartbeatInterval);
  }

  private startPolling(): void {
    this.pollTimer = setInterval(async () => {
      if (!this.running) return;
      await this.tryClaimAndExecute();
    }, this.pollInterval);
  }

  private startRecovery(): void {
    const recoveryInterval = 60000; // 1 minute
    this.recoveryTimer = setInterval(async () => {
      try {
        const timeoutSeconds = 120; // 2 minutes
        const recovered = await RustBridge.recoverDeadWorkers(timeoutSeconds);
        if (recovered > 0) {
          console.log(`Recovered ${recovered} executions from dead workers`);
        }
      } catch (error) {
        console.error('Error in recovery loop:', error);
      }
    }, recoveryInterval);
  }

  private async tryClaimAndExecute(): Promise<void> {
    if (this.currentExecutions >= this.maxConcurrent) {
      return;
    }

    try {
      const execution = await RustBridge.claimExecution(this.workerId, this.queues);
      if (execution) {
        console.log(
          `Claimed ${execution.type} execution ${execution.id}: ${execution.function_name}`
        );
        this.executeWithTracking(execution);
      }
    } catch (error) {
      console.error('Error claiming execution:', error);
    }
  }

  private async executeWithTracking(execution: any): Promise<void> {
    this.currentExecutions++;
    try {
      await this.execute(execution);
    } finally {
      this.currentExecutions--;
    }
  }

  private async execute(execution: any): Promise<void> {
    try {
      console.log(`Executing ${execution.type} ${execution.id}: ${execution.function_name}`);

      // Execute based on type
      if (execution.type === 'workflow') {
        // DSL workflows are handled by Rust
        throw new Error('DSL workflows should be executed by Rust core, not Node.js worker');
      } else {
        // Get the function
        const fn = registry.get(execution.function_name);
        if (!fn) {
          throw new Error(`Function not found: ${execution.function_name}`);
        }
        await this.executeFunction(execution, fn);
      }
    } catch (error) {
      console.error(`Error executing ${execution.id}:`, error);
      await this.handleExecutionFailure(execution, error);
    }
  }

  private async executeFunction(execution: any, fn: Function): Promise<void> {
    try {
      const timeout = execution.timeout_seconds || settings.defaultTimeout;
      const timeoutMs = timeout * 1000;

      // Execute with timeout
      const result = await Promise.race([
        fn(...execution.args, ...Object.values(execution.kwargs)),
        new Promise((_, reject) =>
          setTimeout(() => reject(new Error(`Execution timed out after ${timeout}s`)), timeoutMs)
        ),
      ]);

      // Mark as completed
      await RustBridge.completeExecution(execution.id, result);
      console.log(`Execution ${execution.id} completed successfully`);
    } catch (error: any) {
      if (error.message?.includes('timed out')) {
        throw error;
      }
      throw error;
    }
  }

  private async handleExecutionFailure(execution: any, error: unknown): Promise<void> {
    const attempt = execution.attempt + 1;
    const maxRetries = execution.max_retries;

    const err = error as Error;
    const errorData = {
      message: err.message || String(error),
      type: err.name || 'Error',
      stack: err.stack,
    };

    console.error(
      `Execution ${execution.id} failed (attempt ${attempt}/${maxRetries}): ${errorData.message}`
    );

    if (attempt < maxRetries) {
      // Retry
      await RustBridge.failExecution(execution.id, errorData, true);
      console.log(`Execution ${execution.id} will retry`);
    } else {
      // Max retries exhausted
      await RustBridge.failExecution(execution.id, errorData, false);
      console.error(`Execution ${execution.id} failed permanently after ${attempt} attempts`);
    }
  }
}

export async function runWorker(options: WorkerOptions): Promise<void> {
  const worker = new Worker(options);

  try {
    await worker.start();

    // Keep process alive
    await new Promise(() => {});
  } catch (error) {
    console.error('Worker error:', error);
  } finally {
    await worker.stop();
  }
}
