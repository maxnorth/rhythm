/**
 * Utility functions
 */

import { randomBytes } from 'crypto';

export function generateId(prefix: string): string {
  const randomPart = randomBytes(8).toString('hex');
  const timestamp = Date.now().toString(36);
  return `${prefix}_${timestamp}_${randomPart}`;
}

export function getFunctionName(fn: Function): string {
  // Try to get the full module path - in Node.js this is limited
  // We'll use the function name for now, similar to Python's __qualname__
  return fn.name || 'anonymous';
}
