/**
 * Core types for Rhythm
 */

export interface ExecutionConfig {
  name?: string; // Function name (optional, inferred from function if not provided)
  queue?: string;
  retries?: number;
  timeout?: number;
  priority?: number;
  version?: number;
}

export interface ExecutableProxy<TArgs extends any[] = any[], TReturn = any> {
  queue(...args: TArgs): Promise<string>;
  options(opts: Partial<ExecutionConfig>): ExecutableProxy<TArgs, TReturn>;
  call(...args: TArgs): Promise<TReturn>;
  functionName: string;
  config: ExecutionConfig;
}

export interface TaskConfig extends ExecutionConfig {
  queue: string;
}

export interface ExecutionStatus {
  id: string;
  type: 'task' | 'workflow';
  function_name: string;
  queue: string;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'suspended';
  priority: number;
  result?: any;
  error?: any;
  attempt: number;
  max_retries: number;
  created_at: Date;
  claimed_at?: Date;
  completed_at?: Date;
}

export interface HistoryEvent {
  type: 'task' | 'version';
  [key: string]: any;
}

export interface Checkpoint {
  history: HistoryEvent[];
  [key: string]: any;
}
