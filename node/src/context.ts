/**
 * Workflow execution context and replay mechanism
 */

import { AsyncLocalStorage } from 'async_hooks';
import type { Checkpoint, HistoryEvent, SignalPayload } from './types.js';
import type { ActivityProxy } from './decorators.js';
import { generateId } from './utils.js';

const workflowContextStorage = new AsyncLocalStorage<WorkflowExecutionContext>();

export class WorkflowSuspendException extends Error {
  constructor(public commands: any[]) {
    super('Workflow suspended');
    this.name = 'WorkflowSuspendException';
  }
}

export class WorkflowExecutionContext {
  private checkpoint: Checkpoint;
  private history: HistoryEvent[];
  private currentStepIndex: number = 0;
  private _isReplaying: boolean;
  private newCommands: any[] = [];
  private pendingSignals: Record<string, any> = {};

  constructor(
    public executionId: string,
    checkpoint?: Checkpoint
  ) {
    this.checkpoint = checkpoint || { history: [] };
    this.history = this.checkpoint.history || [];
    this._isReplaying = this.history.length > 0;

    console.debug(
      `WorkflowExecutionContext created for ${executionId}, ` +
        `history length: ${this.history.length}, replaying: ${this._isReplaying}`
    );
  }

  get isReplaying(): boolean {
    return this._isReplaying;
  }

  private getNextHistoryEvent(): HistoryEvent | null {
    if (this.currentStepIndex < this.history.length) {
      const event = this.history[this.currentStepIndex];
      this.currentStepIndex++;
      console.debug(`Replaying step ${this.currentStepIndex}: ${event.type}`);
      return event;
    }
    return null;
  }

  async executeActivity(activityProxy: ActivityProxy<any, any>, args: any[]): Promise<any> {
    const historyEvent = this.getNextHistoryEvent();

    if (historyEvent) {
      // REPLAY MODE: return cached result
      if (historyEvent.type !== 'activity') {
        throw new Error('History mismatch: expected activity');
      }
      if (historyEvent.name !== activityProxy.functionName) {
        throw new Error(
          `History mismatch: expected ${activityProxy.functionName}, got ${historyEvent.name}`
        );
      }

      console.debug(`Replaying activity ${activityProxy.functionName}`);
      return historyEvent.result;
    } else {
      // NEW STEP: we've finished replaying, now executing new steps
      this._isReplaying = false;
      const activityExecutionId = generateId('act');

      console.debug(`Suspending workflow to execute activity ${activityProxy.functionName}`);

      // Record command to execute this activity
      this.newCommands.push({
        type: 'activity',
        activity_execution_id: activityExecutionId,
        name: activityProxy.functionName,
        args: args,
        kwargs: {},
        config: activityProxy.config,
      });

      // Suspend workflow execution
      throw new WorkflowSuspendException(this.newCommands);
    }
  }

  async waitForSignal(signalName: string, timeout?: number): Promise<SignalPayload> {
    const historyEvent = this.getNextHistoryEvent();

    if (historyEvent) {
      // REPLAY MODE: return cached signal
      if (historyEvent.type !== 'signal') {
        throw new Error('History mismatch: expected signal');
      }
      if (historyEvent.signal_name !== signalName) {
        throw new Error(
          `History mismatch: expected signal ${signalName}, got ${historyEvent.signal_name}`
        );
      }

      console.debug(`Replaying signal ${signalName}`);
      return historyEvent.payload;
    } else {
      // NEW STEP: we've finished replaying, now executing new steps
      this._isReplaying = false;
      console.debug(`Suspending workflow to wait for signal ${signalName}`);

      // Record command to wait for signal
      this.newCommands.push({
        type: 'wait_signal',
        signal_name: signalName,
        timeout: timeout,
      });

      // Suspend workflow execution
      throw new WorkflowSuspendException(this.newCommands);
    }
  }

  getVersion(changeId: string, minVersion: number, maxVersion: number): number {
    const historyEvent = this.getNextHistoryEvent();

    if (historyEvent) {
      // REPLAY MODE: return cached version
      if (historyEvent.type !== 'version') {
        throw new Error('History mismatch: expected version');
      }
      if (historyEvent.change_id !== changeId) {
        throw new Error(
          `History mismatch: expected change_id ${changeId}, got ${historyEvent.change_id}`
        );
      }

      const version = historyEvent.version;
      console.debug(`Replaying version check ${changeId} = ${version}`);
      return version;
    } else {
      // NEW STEP: record the max version (current version)
      console.debug(`Recording version check ${changeId} = ${maxVersion}`);

      // Add to history immediately (version checks don't suspend)
      this.history.push({
        type: 'version',
        change_id: changeId,
        version: maxVersion,
      });
      this.currentStepIndex++;

      return maxVersion;
    }
  }
}

export function getCurrentWorkflowContext(): WorkflowExecutionContext | undefined {
  return workflowContextStorage.getStore();
}

export function setCurrentWorkflowContext(ctx: WorkflowExecutionContext): void {
  workflowContextStorage.enterWith(ctx);
}

export function clearCurrentWorkflowContext(): void {
  // AsyncLocalStorage doesn't have a clear method, but exiting the context handles it
}

export async function runInWorkflowContext<T>(
  ctx: WorkflowExecutionContext,
  fn: () => Promise<T>
): Promise<T> {
  return workflowContextStorage.run(ctx, fn);
}

// Public API functions for use within workflows

export async function waitForSignal(
  signalName: string,
  timeout?: number
): Promise<SignalPayload> {
  const ctx = getCurrentWorkflowContext();
  if (!ctx) {
    throw new Error('waitForSignal() can only be called from within a workflow');
  }
  return ctx.waitForSignal(signalName, timeout);
}

export function getVersion(changeId: string, minVersion: number, maxVersion: number): number {
  const ctx = getCurrentWorkflowContext();
  if (!ctx) {
    throw new Error('getVersion() can only be called from within a workflow');
  }
  return ctx.getVersion(changeId, minVersion, maxVersion);
}

export function isReplaying(): boolean {
  const ctx = getCurrentWorkflowContext();
  if (!ctx) {
    throw new Error('isReplaying() can only be called from within a workflow');
  }
  return ctx.isReplaying;
}
