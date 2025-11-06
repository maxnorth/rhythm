-- Add statement_path to track position in nested AST structures
ALTER TABLE workflow_execution_context
  ADD COLUMN statement_path TEXT DEFAULT '0';

-- Migrate existing data: convert statement_index to path format
UPDATE workflow_execution_context
  SET statement_path = statement_index::text
  WHERE statement_path = '0';

-- We'll keep statement_index for now for backwards compatibility
-- but it will be deprecated in favor of statement_path

-- Insert schema version
INSERT INTO schema_migrations (version) VALUES (4) ON CONFLICT DO NOTHING;
