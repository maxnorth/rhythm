/**
 * Tests for workflow context
 */

import {
  WorkflowExecutionContext,
  WorkflowSuspendException,
  isReplaying,
  getVersion,
  runInWorkflowContext,
} from '../context.js';
import { activity } from '../decorators.js';
import type { HistoryEvent } from '../types.js';

describe('WorkflowExecutionContext', () => {
  describe('replay mechanism', () => {
    it('should replay from history', async () => {
      const history: HistoryEvent[] = [
        {
          type: 'activity',
          name: 'testActivity',
          result: { value: 42 },
        },
      ];

      const ctx = new WorkflowExecutionContext('test-exec-1', { history });

      expect(ctx.isReplaying).toBe(true);

      const testActivity = activity<[], { value: number }>()(async function testActivity() {
        return { value: 100 };
      });

      const result = await ctx.executeActivity(testActivity, []);
      expect(result).toEqual({ value: 42 }); // Should return cached result
      expect(ctx.isReplaying).toBe(true); // Still replaying
    });

    it('should suspend when no history is available', async () => {
      const ctx = new WorkflowExecutionContext('test-exec-2', { history: [] });

      expect(ctx.isReplaying).toBe(false);

      const testActivity = activity<[], { value: number }>()(async function testActivity() {
        return { value: 100 };
      });

      await expect(ctx.executeActivity(testActivity, [])).rejects.toThrow(
        WorkflowSuspendException
      );
    });

    it('should throw on history mismatch', async () => {
      const history: HistoryEvent[] = [
        {
          type: 'activity',
          name: 'wrongActivity',
          result: { value: 42 },
        },
      ];

      const ctx = new WorkflowExecutionContext('test-exec-3', { history });

      const testActivity = activity<[], { value: number }>()(async function testActivity() {
        return { value: 100 };
      });

      await expect(ctx.executeActivity(testActivity, [])).rejects.toThrow('History mismatch');
    });
  });

  describe('version tracking', () => {
    it('should record version for new execution', () => {
      const ctx = new WorkflowExecutionContext('test-exec-4', { history: [] });

      const version = ctx.getVersion('feature_1', 1, 2);
      expect(version).toBe(2); // Should return max version
    });

    it('should replay version from history', () => {
      const history: HistoryEvent[] = [
        {
          type: 'version',
          change_id: 'feature_1',
          version: 1,
        },
      ];

      const ctx = new WorkflowExecutionContext('test-exec-5', { history });

      const version = ctx.getVersion('feature_1', 1, 2);
      expect(version).toBe(1); // Should return historical version
    });
  });

  describe('context helpers', () => {
    it('should get replaying state', async () => {
      const history: HistoryEvent[] = [
        {
          type: 'activity',
          name: 'testActivity',
          result: { value: 42 },
        },
      ];

      const ctx = new WorkflowExecutionContext('test-exec-6', { history });

      await runInWorkflowContext(ctx, async () => {
        expect(isReplaying()).toBe(true);
      });
    });

    it('should throw when called outside workflow', () => {
      expect(() => isReplaying()).toThrow(
        'isReplaying() can only be called from within a workflow'
      );
    });

    it('should get version in context', async () => {
      const ctx = new WorkflowExecutionContext('test-exec-7', { history: [] });

      await runInWorkflowContext(ctx, async () => {
        const version = getVersion('feature_1', 1, 2);
        expect(version).toBe(2);
      });
    });
  });
});
