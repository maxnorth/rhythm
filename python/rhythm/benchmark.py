"""Benchmark tasks for performance testing.

These functions are automatically registered when RHYTHM_BENCHMARK=1 environment
variable is set. They are used by the `rhythm bench` CLI command.

Note: Function names must match what the benchmark CLI expects:
- __rhythm_bench_noop__
- __rhythm_bench_compute__
- __rhythm_bench_task__
"""

import asyncio
from rhythm import task
from rhythm.benchmark_tasks import bench_task


@task(queue="default")
async def __rhythm_bench_noop__(payload_size: int = 0):
    """No-op task for benchmarking throughput with minimal overhead.

    Args:
        payload_size: Size of dummy payload to allocate (tests serialization overhead)
    """
    if payload_size > 0:
        # Allocate and touch the payload to simulate real work
        _ = "x" * payload_size
    # Return immediately
    return {"status": "ok"}


@task(queue="default")
async def __rhythm_bench_compute__(iterations: int = 1000, payload_size: int = 0):
    """CPU-bound task for benchmarking with computational work.

    Args:
        iterations: Number of computation iterations to perform
        payload_size: Size of dummy payload to allocate
    """
    if payload_size > 0:
        _ = "x" * payload_size

    # Simulate CPU-bound work
    result = 0
    for i in range(iterations):
        result += i ** 2

    return {"result": result, "iterations": iterations}


# Keep old name for backwards compatibility with benchmark CLI
__rhythm_bench_task__ = bench_task
