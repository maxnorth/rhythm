"""Configuration management"""

from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    """Application settings configured via environment variables with WORKFLOWS_ prefix.

    Examples:
        >>> # Set via environment
        >>> export WORKFLOWS_DATABASE_URL="postgresql://localhost/mydb"
        >>> export WORKFLOWS_WORKER_POLL_INTERVAL=0.5
    """

    model_config = SettingsConfigDict(env_prefix="WORKFLOWS_")

    database_url: str = Field(
        default="postgresql://localhost/workflows", description="PostgreSQL connection string"
    )

    worker_poll_interval: float = Field(default=1.0, description="Worker poll interval in seconds")

    worker_max_concurrent: int = Field(
        default=10, description="Maximum concurrent task executions per worker"
    )

    worker_verbose: bool = Field(default=False, description="Enable verbose diagnostic logging")

    default_timeout: int = Field(default=300, description="Default task timeout in seconds")

    default_workflow_timeout: int = Field(
        default=3600, description="Default workflow timeout in seconds"
    )

    default_retries: int = Field(
        default=3, description="Default number of retry attempts for failed tasks"
    )

    default_retry_backoff_base: float = Field(
        default=2.0, description="Base multiplier for exponential backoff between retries"
    )

    default_retry_backoff_max: float = Field(
        default=60.0, description="Maximum delay in seconds between retry attempts"
    )


settings = Settings()
