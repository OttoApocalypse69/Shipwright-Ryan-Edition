BEGIN;
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE users (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), email text NOT NULL UNIQUE, display_name text,
  role text NOT NULL DEFAULT 'user' CHECK (role IN ('user','admin')), created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE oauth_accounts (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  provider text NOT NULL CHECK (provider IN ('github','google','discord','oidc')), provider_account_id text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(), UNIQUE(provider, provider_account_id)
);
CREATE TABLE devices (
  id uuid PRIMARY KEY, user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE, public_key text NOT NULL,
  label text, registered_at timestamptz NOT NULL DEFAULT now(), revoked_at timestamptz, UNIQUE(user_id, public_key)
);
CREATE TABLE games (
  id text PRIMARY KEY, title text NOT NULL, franchise text NOT NULL, description text NOT NULL, achievement_namespace text NOT NULL UNIQUE,
  active boolean NOT NULL DEFAULT true, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE game_variants (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), game_id text NOT NULL REFERENCES games(id), variant_key text NOT NULL,
  original_platform text NOT NULL, preferred_runtime_id text, UNIQUE(game_id, variant_key)
);
CREATE TABLE runtimes (
  id text PRIMARY KEY, display_name text NOT NULL, kind text NOT NULL, distribution_mode text NOT NULL,
  implementation_status text NOT NULL, created_at timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE game_variants ADD CONSTRAINT game_variants_preferred_runtime_fk FOREIGN KEY (preferred_runtime_id) REFERENCES runtimes(id);
CREATE TABLE runtime_compatibility (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), game_variant_id uuid NOT NULL REFERENCES game_variants(id), runtime_id text NOT NULL REFERENCES runtimes(id),
  version_range text NOT NULL DEFAULT '*', status text NOT NULL CHECK (status IN ('UNSUPPORTED','INVESTIGATING','EXPERIMENTAL','SUPPORTED','DEGRADED','BROKEN','DEPRECATED')),
  notes text, updated_by uuid REFERENCES users(id), updated_at timestamptz NOT NULL DEFAULT now(), UNIQUE(game_variant_id, runtime_id, version_range)
);
CREATE TABLE game_installations (
  id uuid PRIMARY KEY, user_id uuid NOT NULL REFERENCES users(id), device_id uuid NOT NULL REFERENCES devices(id), game_variant_id uuid NOT NULL REFERENCES game_variants(id),
  runtime_id text NOT NULL REFERENCES runtimes(id), local_reference_hash text NOT NULL, status text NOT NULL, configured_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE game_sessions (
  id uuid PRIMARY KEY, user_id uuid NOT NULL REFERENCES users(id), device_id uuid NOT NULL REFERENCES devices(id), game_variant_id uuid NOT NULL REFERENCES game_variants(id), runtime_id text NOT NULL REFERENCES runtimes(id),
  state text NOT NULL CHECK (state IN ('REQUESTED','AUTHORIZING','PREPARING','STARTING_RUNTIME','RUNTIME_STARTED','PLAYABLE','ENDING','ENDED','FAILED')),
  requested_at timestamptz NOT NULL, started_at timestamptz, playable_at timestamptz, ended_at timestamptz, duration_ms bigint, playable_precision text CHECK (playable_precision IN ('PLAYABLE_EXACT','PLAYABLE_APPROXIMATE')),
  playable_method text, exit_code integer, launch_result text NOT NULL
);
CREATE TABLE achievements (
  id text PRIMARY KEY, title text NOT NULL, description text NOT NULL, trigger_version integer NOT NULL DEFAULT 1, active boolean NOT NULL DEFAULT true
);
CREATE TABLE achievement_unlocks (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES users(id), achievement_id text NOT NULL REFERENCES achievements(id),
  unlocked_at timestamptz NOT NULL, source_session_id uuid REFERENCES game_sessions(id), client_event_id text NOT NULL, UNIQUE(user_id, achievement_id), UNIQUE(client_event_id)
);
CREATE TABLE achievement_progress (
  user_id uuid NOT NULL REFERENCES users(id), achievement_id text NOT NULL REFERENCES achievements(id), progress jsonb NOT NULL DEFAULT '{}'::jsonb,
  updated_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(user_id, achievement_id)
);
CREATE TABLE treaties (
  id text PRIMARY KEY, title text NOT NULL, active_version_id uuid
);
CREATE TABLE treaty_versions (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), treaty_id text NOT NULL REFERENCES treaties(id), semantic_version text NOT NULL,
  document_hash text NOT NULL, structured_document jsonb NOT NULL, published_at timestamptz, UNIQUE(treaty_id, semantic_version)
);
ALTER TABLE treaties ADD CONSTRAINT treaties_active_version_fk FOREIGN KEY (active_version_id) REFERENCES treaty_versions(id);
CREATE TABLE treaty_parties (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), treaty_version_id uuid NOT NULL REFERENCES treaty_versions(id), user_id uuid REFERENCES users(id), party_role text NOT NULL
);
CREATE TABLE treaty_articles (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), treaty_version_id uuid NOT NULL REFERENCES treaty_versions(id), article_number integer NOT NULL CHECK (article_number BETWEEN 1 AND 17), title text NOT NULL, body text NOT NULL, UNIQUE(treaty_version_id, article_number)
);
CREATE TABLE obligations (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), treaty_article_id uuid NOT NULL REFERENCES treaty_articles(id), obligation_key text NOT NULL, policy jsonb NOT NULL, active boolean NOT NULL DEFAULT true
);
CREATE TABLE ratifications (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), treaty_version_id uuid NOT NULL REFERENCES treaty_versions(id), user_id uuid NOT NULL REFERENCES users(id), accepted_at timestamptz NOT NULL, document_hash text NOT NULL, UNIQUE(treaty_version_id, user_id)
);
CREATE TABLE compliance_events (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES users(id), obligation_id uuid REFERENCES obligations(id), state text NOT NULL CHECK (state IN ('COMPLIANT','WARNING','MATERIAL_BREACH','REMEDIATION','RESTORED')), details jsonb NOT NULL DEFAULT '{}'::jsonb, recorded_at timestamptz NOT NULL DEFAULT now(), recorded_by uuid REFERENCES users(id)
);
CREATE TABLE incidents (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES users(id), code text NOT NULL, severity text NOT NULL, summary text NOT NULL, opened_at timestamptz NOT NULL DEFAULT now(), closed_at timestamptz
);
CREATE TABLE remediations (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), incident_id uuid NOT NULL REFERENCES incidents(id), action text NOT NULL, status text NOT NULL, completed_at timestamptz
);
CREATE TABLE entitlements (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES users(id), entitlement_key text NOT NULL, state text NOT NULL CHECK (state IN ('ACTIVE','SUSPENDED','EXPIRED')),
  granted_at timestamptz NOT NULL DEFAULT now(), UNIQUE(user_id, entitlement_key)
);
CREATE TABLE entitlement_leases (
  id uuid PRIMARY KEY, entitlement_id uuid NOT NULL REFERENCES entitlements(id), device_id uuid NOT NULL REFERENCES devices(id), key_id text NOT NULL,
  issued_at timestamptz NOT NULL, not_before timestamptz NOT NULL, expires_at timestamptz NOT NULL, signed_payload text NOT NULL, signature text NOT NULL
);
CREATE TABLE revocations (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), lease_id uuid REFERENCES entitlement_leases(id), entitlement_id uuid REFERENCES entitlements(id), reason text NOT NULL,
  revoked_at timestamptz NOT NULL DEFAULT now(), revoked_by uuid REFERENCES users(id), CHECK (lease_id IS NOT NULL OR entitlement_id IS NOT NULL)
);
CREATE TABLE releases (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), version text NOT NULL, channel text NOT NULL CHECK (channel IN ('stable','beta','nightly')), manifest_sha256 text NOT NULL,
  manifest_signature text NOT NULL, github_release_url text NOT NULL, published_at timestamptz, UNIQUE(version, channel)
);
CREATE TABLE scope_requests (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), requested_by uuid NOT NULL REFERENCES users(id), game_title text NOT NULL,
  state text NOT NULL CHECK (state IN ('REQUESTED','UNDER_REVIEW','ACCEPTED','IN_PROGRESS','SUPPORTED','REJECTED','DEFERRED')),
  contractual_debt boolean NOT NULL DEFAULT false CHECK (contractual_debt = false), notes text, created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE audit_events (
  id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY, actor_user_id uuid REFERENCES users(id), action text NOT NULL, target_type text NOT NULL, target_id text,
  request_id text, metadata jsonb NOT NULL DEFAULT '{}'::jsonb, created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX game_sessions_user_started_idx ON game_sessions(user_id, requested_at DESC);
CREATE INDEX achievement_unlocks_user_idx ON achievement_unlocks(user_id, unlocked_at DESC);
CREATE INDEX compliance_events_user_idx ON compliance_events(user_id, recorded_at DESC);
CREATE INDEX audit_events_created_idx ON audit_events(created_at DESC);
COMMIT;
