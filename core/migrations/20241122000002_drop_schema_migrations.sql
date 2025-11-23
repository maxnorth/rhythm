-- Drop legacy schema_migrations table
--
-- This table was manually maintained in older migrations but is not actually
-- used by the system. SQLx uses _sqlx_migrations for migration tracking.

DROP TABLE IF EXISTS schema_migrations;
