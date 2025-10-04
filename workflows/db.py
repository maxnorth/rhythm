"""Database connection and utilities"""

import asyncpg
from contextlib import asynccontextmanager
from typing import AsyncIterator, Optional
import logging

from workflows.config import settings

logger = logging.getLogger(__name__)

# Global connection pool
_pool: Optional[asyncpg.Pool] = None


async def get_pool() -> asyncpg.Pool:
    """Get or create the connection pool"""
    global _pool
    if _pool is None:
        _pool = await asyncpg.create_pool(
            settings.database_url,
            min_size=2,
            max_size=20,
            command_timeout=60,
        )
    return _pool


async def close_pool():
    """Close the connection pool"""
    global _pool
    if _pool is not None:
        await _pool.close()
        _pool = None


@asynccontextmanager
async def get_connection() -> AsyncIterator[asyncpg.Connection]:
    """Get a database connection from the pool"""
    pool = await get_pool()
    async with pool.acquire() as conn:
        yield conn


async def run_migrations():
    """Run database migrations"""
    async with get_connection() as conn:
        # Read and execute schema file
        schema_path = __file__.replace("db.py", "schema.sql")
        with open(schema_path, "r") as f:
            schema_sql = f.read()

        await conn.execute(schema_sql)
        logger.info("Database migrations completed successfully")


async def execute_query(query: str, *args, fetch: bool = False):
    """Execute a query and optionally fetch results"""
    async with get_connection() as conn:
        if fetch:
            return await conn.fetch(query, *args)
        else:
            return await conn.execute(query, *args)


async def fetch_one(query: str, *args) -> Optional[asyncpg.Record]:
    """Fetch a single row"""
    async with get_connection() as conn:
        return await conn.fetchrow(query, *args)


async def fetch_all(query: str, *args) -> list[asyncpg.Record]:
    """Fetch all rows"""
    async with get_connection() as conn:
        return await conn.fetch(query, *args)
