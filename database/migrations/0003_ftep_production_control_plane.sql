BEGIN;

-- Operational fields added after the initial domain model. All mutations remain
-- attributable without storing game files, save data, or machine fingerprints.
ALTER TABLE users ADD COLUMN IF NOT EXISTS last_authenticated_at timestamptz;
ALTER TABLE devices ADD COLUMN IF NOT EXISTS last_seen_at timestamptz;
ALTER TABLE devices ADD COLUMN IF NOT EXISTS revoked_reason text;
ALTER TABLE entitlement_leases ADD COLUMN IF NOT EXISTS revoked_at timestamptz;
ALTER TABLE game_sessions ADD COLUMN IF NOT EXISTS client_event_id text;
ALTER TABLE game_sessions ADD COLUMN IF NOT EXISTS reported_at timestamptz;
ALTER TABLE game_sessions ADD COLUMN IF NOT EXISTS correction_note text;

CREATE UNIQUE INDEX IF NOT EXISTS devices_public_key_unique_idx ON devices(public_key);
CREATE UNIQUE INDEX IF NOT EXISTS game_sessions_client_event_unique_idx
  ON game_sessions(client_event_id) WHERE client_event_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS devices_user_registered_idx ON devices(user_id, registered_at DESC);
CREATE INDEX IF NOT EXISTS entitlement_leases_device_expiry_idx ON entitlement_leases(device_id, expires_at DESC);
CREATE INDEX IF NOT EXISTS revocations_entitlement_idx ON revocations(entitlement_id, revoked_at DESC);

CREATE TABLE IF NOT EXISTS achievement_events (
  event_id text PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  device_id uuid REFERENCES devices(id) ON DELETE SET NULL,
  event_type text NOT NULL,
  occurred_at timestamptz NOT NULL,
  source text NOT NULL CHECK (source IN ('SRE','FTEP','ADMIN')),
  schema_version integer NOT NULL DEFAULT 1 CHECK (schema_version > 0),
  payload jsonb NOT NULL DEFAULT '{}'::jsonb,
  received_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS achievement_events_user_occurred_idx ON achievement_events(user_id, occurred_at DESC);

CREATE TABLE IF NOT EXISTS request_idempotency (
  user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  operation text NOT NULL,
  idempotency_key text NOT NULL,
  status_code integer NOT NULL,
  response_body jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, operation, idempotency_key)
);
CREATE INDEX IF NOT EXISTS request_idempotency_created_idx ON request_idempotency(created_at);

CREATE TABLE IF NOT EXISTS device_request_nonces (
  device_id uuid NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  nonce text NOT NULL,
  seen_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (device_id, nonce)
);
CREATE INDEX IF NOT EXISTS device_request_nonces_seen_idx ON device_request_nonces(seen_at);

-- Seed the catalog projection used by production APIs. The canonical catalog
-- package remains the launcher/web source of display metadata; these records
-- provide stable relational targets for sessions and compatibility decisions.
INSERT INTO runtimes(id, display_name, kind, distribution_mode, implementation_status) VALUES
  ('shipwright', 'Shipwright native runtime', 'NATIVE', 'BUNDLED', 'SUPPORTED'),
  ('two-ship', '2 Ship 2 Harkinian native runtime', 'NATIVE', 'BUNDLED', 'SUPPORTED'),
  ('switch-runtime', 'Managed Switch runtime adapter', 'EXTERNAL', 'MANUAL_ONLY', 'SUPPORTED'),
  ('dolphin-compatible', 'Dolphin-compatible runtime adapter', 'EXTERNAL', 'MANUAL_ONLY', 'EXPERIMENTAL'),
  ('cemu-compatible', 'Cemu-compatible runtime adapter', 'EXTERNAL', 'MANUAL_ONLY', 'SUPPORTED'),
  ('n64-runtime', 'Generic N64 runtime adapter', 'EXTERNAL', 'MANUAL_ONLY', 'INVESTIGATING'),
  ('manual-session', 'Manual session report', 'MANUAL', 'NONE', 'SUPPORTED')
ON CONFLICT(id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  kind = EXCLUDED.kind,
  distribution_mode = EXCLUDED.distribution_mode,
  implementation_status = EXCLUDED.implementation_status;

INSERT INTO games(id, title, franchise, description, achievement_namespace) VALUES
  ('zelda-oot', 'The Legend of Zelda: Ocarina of Time', 'zelda', 'A supported native-port path using user-supplied game data.', 'ftep.game.zelda-oot'),
  ('zelda-mm', 'The Legend of Zelda: Majora''s Mask', 'zelda', 'A supported native-port path using user-supplied game data.', 'ftep.game.zelda-mm'),
  ('zelda-skyward-sword', 'The Legend of Zelda: Skyward Sword HD', 'zelda', 'An external-runtime integration using user-supplied game data.', 'ftep.game.zelda-skyward-sword'),
  ('zelda-botw', 'The Legend of Zelda: Breath of the Wild', 'zelda', 'A Windows-first external-runtime integration using user-supplied game data.', 'ftep.game.zelda-botw'),
  ('zelda-totk', 'The Legend of Zelda: Tears of the Kingdom', 'zelda', 'An external-runtime integration using user-supplied game data.', 'ftep.game.zelda-totk'),
  ('zelda-echoes-of-wisdom', 'The Legend of Zelda: Echoes of Wisdom', 'zelda', 'An external-runtime integration using user-supplied game data.', 'ftep.game.zelda-echoes-of-wisdom'),
  ('animal-crossing-new-horizons', 'Animal Crossing: New Horizons', 'animal-crossing', 'An external-runtime integration using user-supplied game data.', 'ftep.game.animal-crossing-new-horizons'),
  ('satisfactory', 'Satisfactory', 'satisfactory', 'A privacy-preserving manual session record for Accord compliance.', 'ftep.game.satisfactory')
ON CONFLICT(id) DO UPDATE SET
  title = EXCLUDED.title,
  franchise = EXCLUDED.franchise,
  description = EXCLUDED.description,
  achievement_namespace = EXCLUDED.achievement_namespace,
  active = true;

INSERT INTO game_variants(id, game_id, variant_key, original_platform, preferred_runtime_id) VALUES
  ('10000000-0000-4000-8000-000000000001', 'zelda-oot', 'n64', 'NINTENDO64', 'shipwright'),
  ('10000000-0000-4000-8000-000000000002', 'zelda-mm', 'n64', 'NINTENDO64', 'two-ship'),
  ('10000000-0000-4000-8000-000000000003', 'zelda-skyward-sword', 'switch', 'SWITCH', 'switch-runtime'),
  ('10000000-0000-4000-8000-000000000004', 'zelda-skyward-sword', 'wii', 'WII', 'dolphin-compatible'),
  ('10000000-0000-4000-8000-000000000005', 'zelda-botw', 'wiiu', 'WII_U', 'cemu-compatible'),
  ('10000000-0000-4000-8000-000000000006', 'zelda-totk', 'switch', 'SWITCH', 'switch-runtime'),
  ('10000000-0000-4000-8000-000000000007', 'zelda-echoes-of-wisdom', 'switch', 'SWITCH', 'switch-runtime'),
  ('10000000-0000-4000-8000-000000000008', 'animal-crossing-new-horizons', 'switch', 'SWITCH', 'switch-runtime'),
  ('10000000-0000-4000-8000-000000000009', 'satisfactory', 'pc', 'PC', 'manual-session')
ON CONFLICT(id) DO UPDATE SET
  game_id = EXCLUDED.game_id,
  variant_key = EXCLUDED.variant_key,
  original_platform = EXCLUDED.original_platform,
  preferred_runtime_id = EXCLUDED.preferred_runtime_id;

WITH compatibility(game_id, variant_key, runtime_id, status) AS (
  VALUES
    ('zelda-oot', 'n64', 'shipwright', 'SUPPORTED'),
    ('zelda-oot', 'n64', 'n64-runtime', 'INVESTIGATING'),
    ('zelda-mm', 'n64', 'two-ship', 'SUPPORTED'),
    ('zelda-mm', 'n64', 'n64-runtime', 'INVESTIGATING'),
    ('zelda-skyward-sword', 'switch', 'switch-runtime', 'SUPPORTED'),
    ('zelda-skyward-sword', 'wii', 'dolphin-compatible', 'EXPERIMENTAL'),
    ('zelda-botw', 'wiiu', 'cemu-compatible', 'SUPPORTED'),
    ('zelda-totk', 'switch', 'switch-runtime', 'SUPPORTED'),
    ('zelda-echoes-of-wisdom', 'switch', 'switch-runtime', 'SUPPORTED'),
    ('animal-crossing-new-horizons', 'switch', 'switch-runtime', 'SUPPORTED'),
    ('satisfactory', 'pc', 'manual-session', 'SUPPORTED')
)
INSERT INTO runtime_compatibility(game_variant_id, runtime_id, version_range, status, notes)
SELECT variant.id, compatibility.runtime_id, '*', compatibility.status, 'Seeded from the versioned FTEP catalog.'
FROM compatibility
JOIN game_variants variant ON variant.game_id = compatibility.game_id AND variant.variant_key = compatibility.variant_key
ON CONFLICT(game_variant_id, runtime_id, version_range) DO UPDATE SET
  status = EXCLUDED.status,
  notes = EXCLUDED.notes,
  updated_at = now();

INSERT INTO achievements(id, title, description, trigger_version) VALUES
  ('FTEP-ACH-0001', 'Ahh, Zelda', 'Hope you had fun playing. Now onto Satisfactory.', 1),
  ('FTEP-ACH-0002', 'Treaty Ratified', 'You actually agreed to this.', 1),
  ('FTEP-ACH-0003', 'Somehow This Needed OAuth', 'You wanted Zelda. We built identity infrastructure.', 1),
  ('FTEP-ACH-0004', 'Legally Supplied Bits', 'FTEP has detected a completely user-provided collection of data.', 1),
  ('FTEP-ACH-0005', 'Registered Gaming Apparatus', 'Your computer is now recognized by the Interpersonal Treaty Authority.', 1),
  ('FTEP-ACH-0006', 'The Invoice Has Come Due', 'Article II would like a word.', 1),
  ('FTEP-ACH-0007', 'FICSIT Employee Onboarding', 'Welcome to the factory. Your free time has been processed.', 1),
  ('FTEP-ACH-0008', 'Article II Enjoyer', 'Industrial cooperation has improved diplomatic relations.', 1),
  ('FTEP-ACH-0009', 'Diplomatic Relations Restored', 'Zelda privileges restored. Try not to ruin this.', 1),
  ('FTEP-ACH-0010', 'PostgreSQL Was Necessary', 'It absolutely was not.', 1),
  ('FTEP-ACH-0011', 'Enterprise Gaming', 'A distributed system was deployed so two people could play video games.', 1),
  ('FTEP-ACH-0012', 'Ryan Moment', 'Engineering could not have reasonably anticipated this.', 1),
  ('FTEP-ACH-0013', 'Fluid Logistics', 'Biological machinery also requires input buffers.', 1),
  ('FTEP-ACH-0014', 'Pipeline Operational', 'Water successfully delivered to operator.', 1)
ON CONFLICT(id) DO UPDATE SET title = EXCLUDED.title, description = EXCLUDED.description, active = true;

INSERT INTO treaties(id, title) VALUES ('FICSIT-ACCORD-0001', 'Great Zelda-Satisfactory Accords')
ON CONFLICT(id) DO UPDATE SET title = EXCLUDED.title;
INSERT INTO treaty_versions(treaty_id, semantic_version, document_hash, structured_document, published_at)
VALUES ('FICSIT-ACCORD-0001', '3.0.0', 'c643230cb5e86779bcbd0aefb0d56a50c1880d7b619372e7ec6d39ef15e9f2a1', '{
  "treatyId": "FICSIT-ACCORD-0001",
  "version": "3.0.0",
  "title": "Great Zelda-Satisfactory Accords",
  "schedule": {"qualifyingMinutes": 30, "expectedMinMinutes": 30, "expectedMaxMinutes": 120, "dailyRecognitionCapMinutes": 240, "realLifeOverrides": true},
  "articles": [
    {"number": 1, "title": "Provider Technical Obligations", "summary": "The Provider maintains SRE in good faith while preserving legal and security boundaries."},
    {"number": 2, "title": "Satisfactory Cooperation", "summary": "Voluntary cooperation sessions of at least 30 minutes qualify; 30 to 120 minutes is the normal range."},
    {"number": 3, "title": "World Preservation", "summary": "Shared worlds and saves are backed up and treated with care."},
    {"number": 4, "title": "Avoidable Repair and Token Cost", "summary": "Avoidable damage may be recorded for humorous, cooperative remediation."},
    {"number": 5, "title": "No Rage Quitting", "summary": "Parties communicate before abandoning a scheduled cooperative activity."},
    {"number": 6, "title": "Good-Faith Participation", "summary": "Participation is collaborative and honest, not mechanically optimized around loopholes."},
    {"number": 7, "title": "Major Infrastructure Change Control", "summary": "Material shared-world changes require notice and agreement."},
    {"number": 8, "title": "Save Integrity", "summary": "SRE never deletes, encrypts, or silently modifies user saves."},
    {"number": 9, "title": "Remediation", "summary": "Breaches use proportionate, documented, non-destructive remediation."},
    {"number": 10, "title": "Scheduling", "summary": "Schedules respect availability, consent, time zones, work, sleep, and real life."},
    {"number": 11, "title": "Hydration Recommendation", "summary": "Long sessions may show a dismissible hydration reminder."},
    {"number": 12, "title": "Anti-Loophole and Good Faith", "summary": "Technical compliance does not override the cooperative purpose of the Accord."},
    {"number": 13, "title": "Entitlement Remedies", "summary": "Suspension only prevents a future FTEP-authorized launch; it never damages files or systems."},
    {"number": 14, "title": "Restoration of Privileges", "summary": "Completed remediation restores future launch eligibility and is recorded transparently."},
    {"number": 15, "title": "Scope Expansion", "summary": "New title requests enter review and do not automatically create contractual debt."},
    {"number": 16, "title": "Reciprocal Performance", "summary": "Each party''s obligations remain proportionate and mutually understood."},
    {"number": 17, "title": "Anti-Crunch and Human Limits", "summary": "No sleep deprivation, continuous play, coercive crunch, or health-compromising behavior is recognized or required."}
  ]
}'::jsonb, now())
ON CONFLICT(treaty_id, semantic_version) DO UPDATE SET
  document_hash = EXCLUDED.document_hash,
  structured_document = EXCLUDED.structured_document,
  published_at = COALESCE(treaty_versions.published_at, EXCLUDED.published_at);
UPDATE treaties
SET active_version_id = (SELECT id FROM treaty_versions WHERE treaty_id = 'FICSIT-ACCORD-0001' AND semantic_version = '3.0.0')
WHERE id = 'FICSIT-ACCORD-0001';

INSERT INTO treaty_articles(treaty_version_id, article_number, title, body)
SELECT version.id, article.number, article.title, article.summary
FROM treaty_versions version
CROSS JOIN (VALUES
  (1, 'Provider Technical Obligations', 'The Provider maintains SRE in good faith while preserving legal and security boundaries.'),
  (2, 'Satisfactory Cooperation', 'Voluntary cooperation sessions of at least 30 minutes qualify; 30 to 120 minutes is the normal range.'),
  (3, 'World Preservation', 'Shared worlds and saves are backed up and treated with care.'),
  (4, 'Avoidable Repair and Token Cost', 'Avoidable damage may be recorded for humorous, cooperative remediation.'),
  (5, 'No Rage Quitting', 'Parties communicate before abandoning a scheduled cooperative activity.'),
  (6, 'Good-Faith Participation', 'Participation is collaborative and honest, not mechanically optimized around loopholes.'),
  (7, 'Major Infrastructure Change Control', 'Material shared-world changes require notice and agreement.'),
  (8, 'Save Integrity', 'SRE never deletes, encrypts, or silently modifies user saves.'),
  (9, 'Remediation', 'Breaches use proportionate, documented, non-destructive remediation.'),
  (10, 'Scheduling', 'Schedules respect availability, consent, time zones, work, sleep, and real life.'),
  (11, 'Hydration Recommendation', 'Long sessions may show a dismissible hydration reminder.'),
  (12, 'Anti-Loophole and Good Faith', 'Technical compliance does not override the cooperative purpose of the Accord.'),
  (13, 'Entitlement Remedies', 'Suspension only prevents a future FTEP-authorized launch; it never damages files or systems.'),
  (14, 'Restoration of Privileges', 'Completed remediation restores future launch eligibility and is recorded transparently.'),
  (15, 'Scope Expansion', 'New title requests enter review and do not automatically create contractual debt.'),
  (16, 'Reciprocal Performance', 'Each party''s obligations remain proportionate and mutually understood.'),
  (17, 'Anti-Crunch and Human Limits', 'No sleep deprivation, continuous play, coercive crunch, or health-compromising behavior is recognized or required.')
) AS article(number, title, summary)
WHERE version.treaty_id = 'FICSIT-ACCORD-0001' AND version.semantic_version = '3.0.0'
ON CONFLICT(treaty_version_id, article_number) DO UPDATE SET title = EXCLUDED.title, body = EXCLUDED.body;

COMMIT;
