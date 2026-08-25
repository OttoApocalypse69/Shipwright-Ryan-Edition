BEGIN;

INSERT INTO achievements(id, title, description, trigger_version)
VALUES ('FTEP-ACH-0015', 'That Is Not Zelda', 'Animal Crossing support has been requested under the Zelda modernization programme.', 1)
ON CONFLICT(id) DO UPDATE SET title = EXCLUDED.title, description = EXCLUDED.description, active = true;

COMMIT;
