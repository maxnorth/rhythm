-- Merge args and kwargs into a single inputs column
-- This simplifies the API by using a single inputs object instead of Python-style args/kwargs

-- Add new inputs column (combining args and kwargs)
ALTER TABLE executions
ADD COLUMN inputs jsonb NOT NULL DEFAULT '{}'::jsonb;

-- Migrate existing data: merge args and kwargs into inputs
-- If kwargs is non-empty, use it; otherwise use args
UPDATE executions
SET inputs = CASE
    WHEN kwargs != '{}'::jsonb THEN kwargs
    WHEN args != '[]'::jsonb THEN jsonb_build_object('args', args)
    ELSE '{}'::jsonb
END;

-- Drop old columns
ALTER TABLE executions
DROP COLUMN args,
DROP COLUMN kwargs;
