"""Benchmark task functions - separate module to avoid issues"""

import asyncio
from rhythm import task


@task(queue="default")
async def bench_task(payload_size: int = 0):
    """No-op task for workflow benchmarking.

    Args:
        payload_size: Size of dummy payload to allocate
    """
    if payload_size > 0:
        _ = "x" * payload_size

    # Minimal work
    await asyncio.sleep(0.001)  # 1ms simulated I/O
    return {"status": "ok"}
