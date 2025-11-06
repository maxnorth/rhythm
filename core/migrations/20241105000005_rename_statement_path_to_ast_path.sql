-- Rename statement_path to ast_path
-- The path tracks position in the AST (Abstract Syntax Tree), not just statements

ALTER TABLE workflow_execution_context
RENAME COLUMN statement_path TO ast_path;
