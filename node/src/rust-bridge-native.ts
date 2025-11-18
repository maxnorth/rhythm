/**
 * Bridge to Rust core - Node.js implementation using NAPI bindings
 */

import { createRequire } from 'module';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { generateId } from './utils.js';

const require = createRequire(import.meta.url);
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Import the native addon from the main package
let bindings: any;
try {
  // Load the NAPI-generated index.js which handles platform detection
  bindings = require(join(__dirname, '..', 'index.js'));
} catch (error) {
  console.warn('Native bindings not available. Using stub implementation.');
  console.warn('To build: npm run build:native');
  bindings = null;
}

export interface CreateExecutionParams {
  execType: string;
  functionName: string;
  queue: string;
  inputs: Record<string, any>;
  maxRetries: number;
  parentWorkflowId?: string;
}

class RustBridgeImpl {
  async createExecution(params: CreateExecutionParams): Promise<string> {
    if (!bindings) {
      // Stub implementation
      const executionId = generateId(params.execType.substring(0, 3));
      console.warn('RustBridge.createExecution is a stub - using native bindings');
      return executionId;
    }

    return await bindings.createExecution(
      params.execType,
      params.functionName,
      params.queue,
      JSON.stringify(params.inputs),
      params.maxRetries,
      params.parentWorkflowId ?? null
    );
  }

  async getExecution(executionId: string): Promise<any> {
    if (!bindings) {
      console.warn('RustBridge.getExecution is a stub - using native bindings');
      return null;
    }

    const result = await bindings.getExecution(executionId);
    return result ? JSON.parse(result) : null;
  }

  async failExecution(executionId: string, error: any, retry: boolean): Promise<void> {
    if (!bindings) {
      console.warn('RustBridge.failExecution is a stub - using native bindings');
      return;
    }

    await bindings.failExecution(executionId, JSON.stringify(error), retry);
  }

  async completeExecution(executionId: string, result: any): Promise<void> {
    if (!bindings) {
      console.warn('RustBridge.completeExecution is a stub - using native bindings');
      return;
    }

    await bindings.completeExecution(executionId, JSON.stringify(result));
  }

  async claimExecution(workerId: string, queues: string[]): Promise<any> {
    if (!bindings) {
      console.warn('RustBridge.claimExecution is a stub - using native bindings');
      return null;
    }

    const result = await bindings.claimExecution(workerId, queues);
    return result ? JSON.parse(result) : null;
  }

  async migrate(): Promise<void> {
    if (!bindings) {
      throw new Error('Native bindings required for migrations. Build with: cd ../node-bindings && npm run build');
    }

    await bindings.migrate();
  }

  isAvailable(): boolean {
    return bindings !== null;
  }
}

export const RustBridge = new RustBridgeImpl();
