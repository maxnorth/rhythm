/**
 * Decorators for defining jobs, activities, and workflows
 */

import { queueExecution } from './client.js';
import { getCurrentWorkflowContext } from './context.js';
import { registerFunction } from './registry.js';
import { settings } from './config.js';
import type {
  ExecutableProxy,
  JobConfig,
  ActivityConfig,
  WorkflowConfig,
  ExecutionConfig,
} from './types.js';

type AsyncFunction<TArgs extends any[] = any[], TReturn = any> = (
  ...args: TArgs
) => Promise<TReturn>;

export class BaseExecutableProxy<TArgs extends any[] = any[], TReturn = any>
  implements ExecutableProxy<TArgs, TReturn>
{
  public functionName: string;
  public config: ExecutionConfig;

  constructor(
    protected fn: AsyncFunction<TArgs, TReturn>,
    protected execType: string,
    config: ExecutionConfig
  ) {
    // Use provided name or infer from function
    this.functionName = config.name || fn.name;

    if (!this.functionName) {
      throw new Error(
        `${execType} decorator: function has no name. Either use a named function or provide a 'name' in config.`
      );
    }

    this.config = {
      retries: config.retries ?? settings.defaultRetries,
      timeout: config.timeout,
      priority: config.priority ?? 5,
      ...config,
    };

    registerFunction(this.functionName, fn);
  }

  options(opts: Partial<ExecutionConfig>): ExecutableProxy<TArgs, TReturn> {
    const newConfig = { ...this.config, ...opts };
    return new BaseExecutableProxy(this.fn, this.execType, newConfig);
  }

  async queue(...args: TArgs): Promise<string> {
    return queueExecution({
      execType: this.execType,
      functionName: this.functionName,
      args: args,
      queue: this.config.queue!,
      priority: this.config.priority,
      maxRetries: this.config.retries,
      timeoutSeconds: this.config.timeout,
    });
  }

  async run(...args: TArgs): Promise<TReturn> {
    const ctx = getCurrentWorkflowContext();
    if (!ctx) {
      throw new Error(
        `${this.execType}.run() can only be called from within a workflow. Use .queue() to run standalone.`
      );
    }

    return ctx.executeActivity(this as any, args);
  }

  async call(...args: TArgs): Promise<TReturn> {
    return this.fn(...args);
  }
}

export class JobProxy<TArgs extends any[] = any[], TReturn = any> extends BaseExecutableProxy<
  TArgs,
  TReturn
> {
  constructor(fn: AsyncFunction<TArgs, TReturn>, config: JobConfig) {
    if (!config.queue) {
      throw new Error('@job decorator requires a "queue" parameter');
    }
    const fullConfig = {
      ...config,
      timeout: config.timeout ?? settings.defaultTimeout,
    };
    super(fn, 'job', fullConfig);
  }
}

export class ActivityProxy<TArgs extends any[] = any[], TReturn = any> extends BaseExecutableProxy<
  TArgs,
  TReturn
> {
  constructor(fn: AsyncFunction<TArgs, TReturn>, config: ActivityConfig) {
    const fullConfig = {
      ...config,
      timeout: config.timeout ?? settings.defaultTimeout,
    };
    super(fn, 'activity', fullConfig);
  }
}

export class WorkflowProxy<TArgs extends any[] = any[], TReturn = any> extends BaseExecutableProxy<
  TArgs,
  TReturn
> {
  public version: number;

  constructor(fn: AsyncFunction<TArgs, TReturn>, config: WorkflowConfig) {
    if (!config.queue) {
      throw new Error('@workflow decorator requires a "queue" parameter');
    }
    const fullConfig = {
      ...config,
      timeout: config.timeout ?? settings.defaultWorkflowTimeout,
    };
    super(fn, 'workflow', fullConfig);
    this.version = config.version ?? 1;
  }
}

// Decorator functions

export function job<TArgs extends any[] = any[], TReturn = any>(
  config: JobConfig
): (fn: AsyncFunction<TArgs, TReturn>) => JobProxy<TArgs, TReturn> {
  return (fn: AsyncFunction<TArgs, TReturn>) => {
    return new JobProxy(fn, config);
  };
}

export function activity<TArgs extends any[] = any[], TReturn = any>(
  config: ActivityConfig = {}
): (fn: AsyncFunction<TArgs, TReturn>) => ActivityProxy<TArgs, TReturn> {
  return (fn: AsyncFunction<TArgs, TReturn>) => {
    return new ActivityProxy(fn, config);
  };
}

export function workflow<TArgs extends any[] = any[], TReturn = any>(
  config: WorkflowConfig
): (fn: AsyncFunction<TArgs, TReturn>) => WorkflowProxy<TArgs, TReturn> {
  return (fn: AsyncFunction<TArgs, TReturn>) => {
    return new WorkflowProxy(fn, config);
  };
}
