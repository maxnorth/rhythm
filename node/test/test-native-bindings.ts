#!/usr/bin/env node
/**
 * Simple test to verify native bindings are loaded
 */

import { createRequire } from 'module';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const require = createRequire(import.meta.url);
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

console.log('======================================================================');
console.log('Testing Native Bindings Load');
console.log('======================================================================\n');

try {
  // Load from the package root (where NAPI generates index.js)
  const bindings = require(join(__dirname, 'index.js'));

  console.log('✅ Native bindings loaded successfully!\n');
  console.log('Available functions:');

  const functions = [
    'createExecution',
    'claimExecution',
    'completeExecution',
    'failExecution',
    'getExecution',
    'getWorkflowTasks',
    'migrate'
  ];

  let allPresent = true;
  for (const fn of functions) {
    const present = typeof bindings[fn] === 'function';
    console.log(`  ${present ? '✅' : '❌'} ${fn}: ${typeof bindings[fn]}`);
    if (!present) allPresent = false;
  }

  console.log('\n======================================================================');
  if (allPresent) {
    console.log('✅ SUCCESS: All native functions are available!');
    console.log('======================================================================\n');
    console.log('Next steps:');
    console.log('  1. Set up PostgreSQL database');
    console.log('  2. export RHYTHM_DATABASE_URL="postgresql://user:pass@localhost/rhythm"');
    console.log('  3. Run migrations: node dist/cli.js migrate');
    console.log('  4. Start worker: node dist/cli.js worker -q default');
  } else {
    console.log('❌ FAILURE: Some functions are missing');
  }
  console.log('======================================================================');

} catch (error) {
  console.error('❌ Failed to load native bindings');
  console.error(`Error: ${(error as Error).message}`);
  console.error('\nMake sure to build the native bindings:');
  console.error('  cd native && npm run build');
  process.exit(1);
}
