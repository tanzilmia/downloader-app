-- Existing jobs retain their mock provenance. New output metadata is optional.
-- The JSON payload remains the authoritative, versioned wire snapshot.
UPDATE jobs SET payload = json_set(payload, '$.output', NULL)
WHERE json_extract(payload, '$.isMock') = 0
  AND json_type(payload, '$.output') IS NULL;
CREATE INDEX jobs_completed ON jobs(status) WHERE status = 'completed';
