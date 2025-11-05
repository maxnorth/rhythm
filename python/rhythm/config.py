"""Configuration management"""

from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    """Application settings"""

    model_config = SettingsConfigDict(env_prefix="WORKFLOWS_")

    # Database
    database_url: str = "postgresql://localhost/workflows"

    # Worker settings
    worker_heartbeat_interval: int = 5  # seconds
    worker_heartbeat_timeout: int = 30  # seconds
    worker_poll_interval: float = 1.0  # seconds (can be fractional for fast polling in tests)
    worker_max_concurrent: int = 10  # max concurrent executions per worker

    # Execution defaults
    default_timeout: int = 300  # seconds
    default_workflow_timeout: int = 3600  # seconds
    default_retries: int = 3
    default_retry_backoff_base: float = 2.0  # seconds
    default_retry_backoff_max: float = 60.0  # seconds


settings = Settings()
