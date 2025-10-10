"""Benchmark jobs and workflows for performance testing.

These functions are automatically registered when CURRANT_BENCHMARK=1 environment
variable is set. They are used by the `currant bench` CLI command.

Note: Function names must match what the benchmark CLI expects:
- __currant_bench_noop__
- __currant_bench_compute__
- __currant_bench_activity__
- __currant_bench_workflow__
"""

import asyncio
from currant import job, workflow
from currant.benchmark_activities import bench_activity


@job(queue="default")
async def __currant_bench_noop__(payload_size: int = 0):
    """No-op job for benchmarking throughput with minimal overhead.

    Args:
        payload_size: Size of dummy payload to allocate (tests serialization overhead)
    """
    if payload_size > 0:
        # Allocate and touch the payload to simulate real work
        _ = "x" * payload_size
    # Return immediately
    return {"status": "ok"}


@job(queue="default")
async def __currant_bench_compute__(iterations: int = 1000, payload_size: int = 0):
    """CPU-bound job for benchmarking with computational work.

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


@workflow(queue="default")
async def __currant_bench_workflow__(activity_count: int = 3, payload_size: int = 0):
    """Benchmark workflow that spawns multiple activities.

    Args:
        activity_count: Number of activities to spawn
        payload_size: Size of dummy payload for each activity
    """
    results = []

    for i in range(activity_count):
        result = await bench_activity.run(payload_size)
        results.append(result)

    return {
        "activity_count": activity_count,
        "results": results,
    }


# Keep old name for backwards compatibility
__currant_bench_activity__ = bench_activity
