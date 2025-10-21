/**
 * Decorators for defining tasks
 */

import { queueExecution } from './client.js';
import { registerFunction } from './registry.js';
import { settings } from './config.js';
import type {
  ExecutableProxy,
  TaskConfig,
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

  async call(...args: TArgs): Promise<TReturn> {
    return this.fn(...args);
  }
}

export class TaskProxy<TArgs extends any[] = any[], TReturn = any> extends BaseExecutableProxy<
  TArgs,
  TReturn
> {
  constructor(fn: AsyncFunction<TArgs, TReturn>, config: TaskConfig) {
    if (!config.queue) {
      throw new Error('@task decorator requires a "queue" parameter');
    }
    const fullConfig = {
      ...config,
      timeout: config.timeout ?? settings.defaultTimeout,
    };
    super(fn, 'task', fullConfig);
  }
}

// Decorator functions

export function task<TArgs extends any[] = any[], TReturn = any>(
  config: TaskConfig
): (fn: AsyncFunction<TArgs, TReturn>) => TaskProxy<TArgs, TReturn> {
  return (fn: AsyncFunction<TArgs, TReturn>) => {
    return new TaskProxy(fn, config);
  };
}
