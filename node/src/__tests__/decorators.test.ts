/**
 * Tests for decorators
 */

import { job, activity, workflow } from '../decorators.js';
import { registry } from '../registry.js';

describe('Decorators', () => {
  describe('job', () => {
    it('should create a job proxy', () => {
      const testJob = job<[string], string>({ queue: 'test' })(async function testJob(msg: string) {
        return `processed: ${msg}`;
      });

      expect(testJob).toBeDefined();
      expect(testJob.functionName).toBe('testJob');
    });

    it('should register the function', () => {
      const testJob2 = job<[string], string>({ queue: 'test' })(async function testJob2(msg: string) {
        return `processed: ${msg}`;
      });

      expect(registry.has(testJob2.functionName)).toBe(true);
    });

    it('should throw error if queue is missing', () => {
      expect(() => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        job<[string], string>({} as any)(async function testJob3(msg: string) {
          return `processed: ${msg}`;
        });
      }).toThrow('@job decorator requires a "queue" parameter');
    });

    it('should allow direct call', async () => {
      const testJob = job<[string], string>({ queue: 'test' })(async function testJob4(msg: string) {
        return `processed: ${msg}`;
      });

      const result = await testJob.call('hello');
      expect(result).toBe('processed: hello');
    });
  });

  describe('activity', () => {
    it('should create an activity proxy', () => {
      const testActivity = activity<[number, number], number>({ retries: 3 })(
        async function testActivity(a: number, b: number) {
          return a + b;
        }
      );

      expect(testActivity).toBeDefined();
      expect(testActivity.functionName).toBe('testActivity');
    });

    it('should work without config', () => {
      const testActivity = activity<[], void>()(async function testActivity2() {
        // do nothing
      });

      expect(testActivity).toBeDefined();
    });

    it('should allow direct call', async () => {
      const testActivity = activity<[number, number], number>()(
        async function testActivity3(a: number, b: number) {
          return a + b;
        }
      );

      const result = await testActivity.call(5, 3);
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
      const testJob = job<[string], string>({ queue: 'test', priority: 5 })(
        async function testJob5(msg: string) {
          return msg;
        }
      );

      const highPriorityJob = testJob.options({ priority: 10 });

      // Original should be unchanged
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      expect((testJob as any).config.priority).toBe(5);

      // New instance should have new priority
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      expect((highPriorityJob as any).config.priority).toBe(10);
    });
  });
});
