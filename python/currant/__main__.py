"""
Currant CLI entry point

This module provides a thin wrapper around the Rust CLI implementation.
All CLI logic is implemented in Rust core for consistency across language adapters.
"""

import sys


def main():
    """
    Entry point for the Currant CLI.

    This is intentionally minimal - all CLI parsing and logic happens in Rust.
    We normalize sys.argv and pass it to Rust for parsing.
    """
    # Normalize argv for Rust CLI
    # When run as 'python -m currant migrate', Python sets:
    # sys.argv = ['/path/to/currant/__main__.py', 'migrate', ...]
    # We want: args = ['currant', 'migrate', ...]
    args = sys.argv.copy()
    args[0] = 'currant'

    # Check for 'worker bench' subcommand first (more specific)
    if len(args) > 2 and args[1] == "worker" and args[2] == "bench":
        # Python handles 'worker bench' subcommand
        from currant.rust_bridge import RustBridge
        import argparse

        # Detect Python executable
        python_cmd = sys.executable or "python"

        # Build worker command: python -m currant worker --import currant.benchmark
        worker_cmd = [python_cmd, "-m", "currant", "worker", "--import", "currant.benchmark"]

        # Parse benchmark arguments
        parser = argparse.ArgumentParser(prog='currant worker bench')
        parser.add_argument('--workers', type=int, default=10)
        parser.add_argument('--tasks', type=int, default=0)
        parser.add_argument('--workflows', type=int, default=0)
        parser.add_argument('--task-type', default='noop')
        parser.add_argument('--payload-size', type=int, default=0)
        parser.add_argument('--tasks-per-workflow', type=int, default=3)
        parser.add_argument('--queues', default='default')
        parser.add_argument('--queue-distribution', default=None)
        parser.add_argument('--duration', default=None)
        parser.add_argument('--rate', type=float, default=None)
        parser.add_argument('--compute-iterations', type=int, default=1000)
        parser.add_argument('--warmup-percent', type=float, default=0.0)

        try:
            bench_args = parser.parse_args(args[3:])  # Skip 'currant', 'worker', and 'bench'

            # Call Rust benchmark function directly
            RustBridge.run_benchmark(
                worker_command=worker_cmd,
                workers=bench_args.workers,
                tasks=bench_args.tasks,
                workflows=bench_args.workflows,
                task_type=bench_args.task_type,
                payload_size=bench_args.payload_size,
                tasks_per_workflow=bench_args.tasks_per_workflow,
                queues=bench_args.queues,
                compute_iterations=bench_args.compute_iterations,
                warmup_percent=bench_args.warmup_percent,
                queue_distribution=bench_args.queue_distribution,
                duration=bench_args.duration,
                rate=bench_args.rate,
            )
        except KeyboardInterrupt:
            print("\nInterrupted")
            sys.exit(130)
        except Exception as e:
            print(f"Error: {e}", file=sys.stderr)
            sys.exit(1)
    elif len(args) > 1 and args[1] == "worker":
        # Regular worker command - needs Python-specific logic
        import asyncio
        from currant.worker import run_worker

        # Parse worker args
        queues = []
        worker_id = None
        import_modules = []

        i = 2
        while i < len(args):
            if args[i] in ("-q", "--queue"):
                if i + 1 < len(args):
                    queues.append(args[i + 1])
                    i += 2
                else:
                    print("Error: --queue requires a value", file=sys.stderr)
                    sys.exit(1)
            elif args[i] == "--worker-id":
                if i + 1 < len(args):
                    worker_id = args[i + 1]
                    i += 2
                else:
                    print("Error: --worker-id requires a value", file=sys.stderr)
                    sys.exit(1)
            elif args[i] in ("-m", "--import"):
                if i + 1 < len(args):
                    import_modules.append(args[i + 1])
                    i += 2
                else:
                    print("Error: --import requires a value", file=sys.stderr)
                    sys.exit(1)
            else:
                print(f"Error: Unknown argument: {args[i]}", file=sys.stderr)
                sys.exit(1)

        if not queues:
            print("Error: At least one queue is required (-q/--queue)", file=sys.stderr)
            sys.exit(1)

        # Auto-import benchmark module if CURRANT_BENCHMARK=1
        import os
        if os.environ.get("CURRANT_BENCHMARK") == "1":
            try:
                import currant.benchmark
                print("✓ Benchmark functions registered")
            except ImportError as e:
                print(f"Warning: Failed to import benchmark module: {e}", file=sys.stderr)

        # Import modules to register decorated functions
        for module_name in import_modules:
            try:
                __import__(module_name)
                print(f"Imported module: {module_name}")
            except ImportError as e:
                print(f"Failed to import {module_name}: {e}", file=sys.stderr)
                sys.exit(1)

        print(f"Starting worker for queues: {', '.join(queues)}")

        # Run the worker
        async def _run():
            try:
                await run_worker(queues, worker_id)
            except KeyboardInterrupt:
                print("\nShutting down worker...")

        try:
            asyncio.run(_run())
        except KeyboardInterrupt:
            print("\nInterrupted")
            sys.exit(130)
    else:
        # All other commands handled by Rust CLI
        from currant.rust_bridge import RustBridge

        try:
            RustBridge.run_cli(args)
        except KeyboardInterrupt:
            print("\nInterrupted")
            sys.exit(130)
        except Exception as e:
            print(f"Error: {e}", file=sys.stderr)
            sys.exit(1)


if __name__ == "__main__":
    main()
