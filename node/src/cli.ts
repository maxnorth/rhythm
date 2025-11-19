#!/usr/bin/env node
/**
 * Command-line interface for Rhythm (Node.js)
 */

import { Command } from 'commander';
import { getExecutionStatus } from './client.js';
import { RustBridge } from './rust-bridge-native.js';

const program = new Command();

program
  .name('rhythm')
  .description('Rhythm - A lightweight durable execution framework')
  .version('0.1.0');

program
  .command('worker')
  .description('Run a worker to process tasks and workflows')
  .requiredOption('-q, --queue <queues...>', 'Queue(s) to process')
  .option('-m, --import <modules...>', 'Module(s) to import')
  .option('--worker-id <id>', 'Worker ID (auto-generated if not provided)')
  .action(async (options) => {
    const queues = options.queue;
    const imports = options.import || [];
    const workerId = options.workerId;

    // Import modules to register functions
    for (const modulePath of imports) {
      try {
        await import(modulePath);
        console.log(`Imported module: ${modulePath}`);
      } catch (error) {
        console.error(`Failed to import ${modulePath}:`, error);
        process.exit(1);
      }
    }

    console.log(`Starting worker for queues: ${queues.join(', ')}`);

    try {
      const { runWorker } = await import('./worker.js');
      await runWorker({ queues, workerId });
    } catch (error) {
      console.error('Worker error:', error);
      process.exit(1);
    }
  });

program
  .command('status')
  .description('Get the status of an execution')
  .argument('<execution-id>', 'Execution ID')
  .action(async (executionId) => {
    try {
      const result = await getExecutionStatus(executionId);
      if (result) {
        console.log(`Execution: ${result.id}`);
        console.log(`Type: ${result.type}`);
        console.log(`Function: ${result.function_name}`);
        console.log(`Queue: ${result.queue}`);
        console.log(`Status: ${result.status}`);
        console.log(`Attempts: ${result.attempt}/3`);
        console.log(`Created: ${result.created_at}`);

        if (result.completed_at) {
          console.log(`Completed: ${result.completed_at}`);
        }

        if (result.result) {
          console.log('\nResult:');
          console.log(JSON.stringify(result.result, null, 2));
        }

        if (result.error) {
          console.log('\nError:');
          console.log(result.error);
        }
      } else {
        console.error(`Execution ${executionId} not found`);
        process.exit(1);
      }
    } catch (error) {
      console.error('Failed to get status:', error);
      process.exit(1);
    }
  });

program
  .command('migrate')
  .description('Run database migrations')
  .action(async () => {
    try {
      console.log('Running database migrations...');
      await RustBridge.migrate();
      console.log('✓ Migrations completed successfully');
    } catch (error) {
      console.error('✗ Migration failed:', error);
      process.exit(1);
    }
  });

program.parse();
