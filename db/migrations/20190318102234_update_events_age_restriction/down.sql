ALTER TABLE events
    ALTER COLUMN age_limit TYPE INTEGER USING age_limit::INTEGER;