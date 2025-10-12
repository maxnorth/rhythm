/**
 * Tests for decorators
 */

import { task, workflow } from '../decorators.js';
import { registry } from '../registry.js';

describe('Decorators', () => {
  describe('task', () => {
    it('should create a task proxy', () => {
      const testTask = task<[string], string>({ queue: 'test' })(async function testTask(msg: string) {
        return `processed: ${msg}`;
      });

      expect(testTask).toBeDefined();
      expect(testTask.functionName).toBe('testTask');
    });

    it('should register the function', () => {
      const testTask2 = task<[string], string>({ queue: 'test' })(async function testTask2(msg: string) {
        return `processed: ${msg}`;
      });

      expect(registry.has(testTask2.functionName)).toBe(true);
    });

    it('should throw error if queue is missing', () => {
      expect(() => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        task<[string], string>({} as any)(async function testTask3(msg: string) {
          return `processed: ${msg}`;
        });
      }).toThrow('@task decorator requires a "queue" parameter');
    });

    it('should allow direct call', async () => {
      const testTask = task<[string], string>({ queue: 'test' })(async function testTask4(msg: string) {
        return `processed: ${msg}`;
      });

      const result = await testTask.call('hello');
      expect(result).toBe('processed: hello');
    });

    it('should allow direct call with multiple params', async () => {
      const testTask = task<[number, number], number>({ queue: 'test' })(
        async function testTask5(a: number, b: number) {
          return a + b;
        }
      );

      const result = await testTask.call(5, 3);
      expect(result).toBe(8);
    });
  });

  describe('workflow', () => {
    it('should create a workflow proxy', () => {
      const testWorkflow = workflow<[string], { status: string }>({
        queue: 'test-workflows',
        version: 1,
      })(async function testWorkflow() {
        return { status: 'completed' };
      });

      expect(testWorkflow).toBeDefined();
      expect(testWorkflow.functionName).toBe('testWorkflow');
      expect(testWorkflow.version).toBe(1);
    });

    it('should throw error if queue is missing', () => {
      expect(() => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        workflow<[string], any>({ version: 1 } as any)(async function testWorkflow2() {
          return { status: 'completed' };
        });
      }).toThrow('@workflow decorator requires a "queue" parameter');
    });

    it('should default version to 1', () => {
      const testWorkflow = workflow<[string], { status: string }>({
        queue: 'test-workflows',
      })(async function testWorkflow3() {
        return { status: 'completed' };
      });

      expect(testWorkflow.version).toBe(1);
    });
  });

  describe('options', () => {
    it('should allow modifying options', () => {
      const testTask = task<[string], string>({ queue: 'test', priority: 5 })(
        async function testTask6(msg: string) {
          return msg;
        }
      );

      const highPriorityTask = testTask.options({ priority: 10 });

      // Original should be unchanged
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      expect((testTask as any).config.priority).toBe(5);

      // New instance should have new priority
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      expect((highPriorityTask as any).config.priority).toBe(10);
    });
  });
});
