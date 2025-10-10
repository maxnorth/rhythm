"""Benchmark activity functions - separate module to avoid issues"""

import asyncio
from currant import activity


@activity()
async def bench_activity(payload_size: int = 0):
    """No-op activity for workflow benchmarking.

    Args:
        payload_size: Size of dummy payload to allocate
    """
    if payload_size > 0:
        _ = "x" * payload_size

    # Minimal work
    await asyncio.sleep(0.001)  # 1ms simulated I/O
    return {"status": "ok"}
