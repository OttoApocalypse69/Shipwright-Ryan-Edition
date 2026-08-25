//! Offline-first achievement engine. Unlocks are committed to local SQLite
//! before an overlay is queued or any asynchronous FTEP synchronization runs.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sre_core::GameId;
use sre_overlay::{OverlayKind, OverlayNotification, OverlayQueue};
use sre_runtime::RuntimeEvent;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementDefinition {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
}

pub const ACHIEVEMENTS: &[AchievementDefinition] = &[
    AchievementDefinition {
        id: "treaty-ratified",
        title: "Treaty Ratified",
        description: "You actually agreed to this.",
    },
    AchievementDefinition {
        id: "somehow-this-needed-oauth",
        title: "Somehow This Needed OAuth",
        description: "You wanted games. We built identity infrastructure.",
    },
    AchievementDefinition {
        id: "registered-gaming-apparatus",
        title: "Registered Gaming Apparatus",
        description: "Your computer is now recognized by the Interpersonal Treaty Authority.",
    },
    AchievementDefinition {
        id: "legally-supplied-bits",
        title: "Legally Supplied Bits",
        description: "FTEP has detected a completely user-provided collection of data.",
    },
    AchievementDefinition {
        id: "ahh-zelda",
        title: "Ahh, Zelda",
        description: "Hope you had fun playing. Now onto Satisfactory.",
    },
    AchievementDefinition {
        id: "the-invoice-has-come-due",
        title: "The Invoice Has Come Due",
        description: "Article II would like a word.",
    },
    AchievementDefinition {
        id: "ficsit-employee-onboarding",
        title: "FICSIT Employee Onboarding",
        description: "Welcome to the factory. Your free time has been processed.",
    },
    AchievementDefinition {
        id: "that-is-not-zelda",
        title: "That Is Not Zelda",
        description: "Animal Crossing support has been requested under the Zelda modernization programme.",
    },
    AchievementDefinition {
        id: "enterprise-gaming",
        title: "Enterprise Gaming",
        description: "A distributed system was deployed so two people could play video games.",
    },
    AchievementDefinition {
        id: "postgresql-was-necessary",
        title: "PostgreSQL Was Necessary",
        description: "It absolutely was not.",
    },
    AchievementDefinition {
        id: "article-ii-enjoyer",
        title: "Article II Enjoyer",
        description: "Industrial cooperation has improved diplomatic relations.",
    },
    AchievementDefinition {
        id: "diplomatic-relations-restored",
        title: "Diplomatic Relations Restored",
        description: "Gaming privileges restored. Try not to ruin this.",
    },
    AchievementDefinition {
        id: "fluid-logistics",
        title: "Fluid Logistics",
        description: "Biological machinery also requires input buffers.",
    },
    AchievementDefinition {
        id: "pipeline-operational",
        title: "Pipeline Operational",
        description: "Water successfully delivered to operator.",
    },
    AchievementDefinition {
        id: "ryan-moment",
        title: "Ryan Moment",
        description: "Engineering could not have reasonably anticipated this.",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AchievementEvent<'a> {
    Runtime {
        game_id: &'a GameId,
        event: &'a RuntimeEvent,
    },
    GameLaunched {
        game_id: &'a GameId,
        at_unix_ms: u64,
    },
    TreatyRatified {
        at_unix_ms: u64,
    },
    OAuthCompleted {
        at_unix_ms: u64,
    },
    DeviceRegistered {
        at_unix_ms: u64,
    },
    GameDataValidated {
        at_unix_ms: u64,
    },
    ArticleIIActivated {
        at_unix_ms: u64,
    },
    SatisfactorySessionQualified {
        at_unix_ms: u64,
    },
    HydrationAcknowledged {
        acknowledgements: u32,
        at_unix_ms: u64,
    },
    BackendMigrationApplied {
        at_unix_ms: u64,
    },
    ControlPlaneHealthy {
        at_unix_ms: u64,
    },
    RelationsRestored {
        at_unix_ms: u64,
    },
    ClassifiedRyanMoment {
        code: &'a str,
        at_unix_ms: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Unlock {
    pub achievement_id: String,
    pub account_id: String,
    pub unlocked_at_unix_ms: u64,
    pub synced: bool,
}

pub struct AchievementEngine {
    connection: Connection,
    overlay: OverlayQueue,
}

impl AchievementEngine {
    pub fn open(path: &Path, overlay: OverlayQueue) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(|error| error.to_string())?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS achievement_unlocks (
               account_id TEXT NOT NULL,
               achievement_id TEXT NOT NULL,
               unlocked_at_unix_ms INTEGER NOT NULL,
               synced INTEGER NOT NULL DEFAULT 0,
               sync_attempts INTEGER NOT NULL DEFAULT 0,
               PRIMARY KEY (account_id, achievement_id)
             );",
            )
            .map_err(|error| error.to_string())?;
        Ok(Self {
            connection,
            overlay,
        })
    }

    pub fn evaluate(
        &mut self,
        account_id: &str,
        event: AchievementEvent<'_>,
    ) -> Result<Vec<Unlock>, String> {
        let candidates = candidates(event);
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let mut unlocked = Vec::new();
        for (achievement_id, at) in candidates {
            let changed = transaction.execute(
                "INSERT OR IGNORE INTO achievement_unlocks(account_id, achievement_id, unlocked_at_unix_ms, synced) VALUES (?1, ?2, ?3, 0)",
                params![account_id, achievement_id, at as i64],
            ).map_err(|error| error.to_string())?;
            if changed == 1 {
                unlocked.push(Unlock {
                    achievement_id: achievement_id.to_owned(),
                    account_id: account_id.to_owned(),
                    unlocked_at_unix_ms: at,
                    synced: false,
                });
            }
        }
        transaction.commit().map_err(|error| error.to_string())?;
        for unlock in &unlocked {
            let definition = definition(&unlock.achievement_id)
                .ok_or_else(|| "achievement seed is missing".to_owned())?;
            self.overlay.push(OverlayNotification {
                id: format!("achievement:{}:{}", account_id, unlock.achievement_id),
                kind: OverlayKind::Achievement,
                title: definition.title.to_owned(),
                message: definition.description.to_owned(),
                created_at_unix_ms: unlock.unlocked_at_unix_ms,
                display_ms: 15_000,
            })?;
        }
        Ok(unlocked)
    }

    pub fn pending_sync(&self, limit: usize) -> Result<Vec<Unlock>, String> {
        let mut statement = self.connection.prepare(
            "SELECT achievement_id, account_id, unlocked_at_unix_ms FROM achievement_unlocks WHERE synced = 0 ORDER BY unlocked_at_unix_ms LIMIT ?1"
        ).map_err(|error| error.to_string())?;
        statement
            .query_map([limit as i64], |row| {
                Ok(Unlock {
                    achievement_id: row.get(0)?,
                    account_id: row.get(1)?,
                    unlocked_at_unix_ms: row.get::<_, i64>(2)? as u64,
                    synced: false,
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn unlocked_count(&self) -> Result<usize, String> {
        self.connection
            .query_row("SELECT COUNT(*) FROM achievement_unlocks", [], |row| {
                row.get::<_, i64>(0)
            })
            .map(|value| value.max(0) as usize)
            .map_err(|error| error.to_string())
    }

    pub fn mark_synced(&self, account_id: &str, achievement_id: &str) -> Result<(), String> {
        self.connection.execute("UPDATE achievement_unlocks SET synced = 1 WHERE account_id = ?1 AND achievement_id = ?2", params![account_id, achievement_id]).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn record_sync_retry(&self, account_id: &str, achievement_id: &str) -> Result<(), String> {
        self.connection.execute("UPDATE achievement_unlocks SET sync_attempts = sync_attempts + 1 WHERE account_id = ?1 AND achievement_id = ?2", params![account_id, achievement_id]).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn is_unlocked(&self, account_id: &str, achievement_id: &str) -> Result<bool, String> {
        self.connection
            .query_row(
                "SELECT 1 FROM achievement_unlocks WHERE account_id = ?1 AND achievement_id = ?2",
                params![account_id, achievement_id],
                |_| Ok(()),
            )
            .optional()
            .map(|value| value.is_some())
            .map_err(|error| error.to_string())
    }
}

fn candidates(event: AchievementEvent<'_>) -> Vec<(&'static str, u64)> {
    match event {
        AchievementEvent::Runtime {
            event: RuntimeEvent::Playable { at_unix_ms, .. },
            ..
        } => vec![("ahh-zelda", *at_unix_ms)],
        AchievementEvent::GameLaunched {
            game_id,
            at_unix_ms,
        } if game_id.as_str() == "animal-crossing-new-horizons" => {
            vec![("that-is-not-zelda", at_unix_ms)]
        }
        AchievementEvent::TreatyRatified { at_unix_ms } => vec![("treaty-ratified", at_unix_ms)],
        AchievementEvent::OAuthCompleted { at_unix_ms } => {
            vec![("somehow-this-needed-oauth", at_unix_ms)]
        }
        AchievementEvent::DeviceRegistered { at_unix_ms } => {
            vec![("registered-gaming-apparatus", at_unix_ms)]
        }
        AchievementEvent::GameDataValidated { at_unix_ms } => {
            vec![("legally-supplied-bits", at_unix_ms)]
        }
        AchievementEvent::ArticleIIActivated { at_unix_ms } => {
            vec![("the-invoice-has-come-due", at_unix_ms)]
        }
        AchievementEvent::SatisfactorySessionQualified { at_unix_ms } => {
            vec![
                ("ficsit-employee-onboarding", at_unix_ms),
                ("article-ii-enjoyer", at_unix_ms),
            ]
        }
        AchievementEvent::HydrationAcknowledged {
            acknowledgements,
            at_unix_ms,
        } => {
            let mut achievements = vec![("fluid-logistics", at_unix_ms)];
            if acknowledgements >= 5 {
                achievements.push(("pipeline-operational", at_unix_ms));
            }
            achievements
        }
        AchievementEvent::BackendMigrationApplied { at_unix_ms } => {
            vec![("postgresql-was-necessary", at_unix_ms)]
        }
        AchievementEvent::ControlPlaneHealthy { at_unix_ms } => {
            vec![("enterprise-gaming", at_unix_ms)]
        }
        AchievementEvent::RelationsRestored { at_unix_ms } => {
            vec![("diplomatic-relations-restored", at_unix_ms)]
        }
        AchievementEvent::ClassifiedRyanMoment {
            code: "FICSIT-0010",
            at_unix_ms,
        } => vec![("ryan-moment", at_unix_ms)],
        _ => vec![],
    }
}

fn definition(id: &str) -> Option<&'static AchievementDefinition> {
    ACHIEVEMENTS.iter().find(|value| value.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sre_runtime::PlayablePrecision;
    use std::time::Instant;

    #[test]
    fn first_playable_unlocks_offline_once_and_reaches_overlay_within_acceptance_budget() {
        let temp = tempfile::tempdir().unwrap();
        let overlay = OverlayQueue::default();
        let mut engine =
            AchievementEngine::open(&temp.path().join("achievements.sqlite3"), overlay.clone())
                .unwrap();
        let game = GameId::new("zelda-botw").unwrap();
        let playable = RuntimeEvent::Playable {
            at_unix_ms: 100,
            precision: PlayablePrecision::Approximate,
        };
        let started = Instant::now();
        let first = engine
            .evaluate(
                "account",
                AchievementEvent::Runtime {
                    game_id: &game,
                    event: &playable,
                },
            )
            .unwrap();
        let elapsed = started.elapsed();
        assert_eq!(first[0].achievement_id, "ahh-zelda");
        assert!(
            elapsed.as_millis() < 250,
            "internal target exceeded: {elapsed:?}"
        );
        assert!(elapsed.as_millis() <= 5_000);
        let notification = overlay.pop().unwrap().unwrap();
        assert_eq!(notification.title, "Ahh, Zelda");
        assert_eq!(notification.display_ms, 15_000);
        assert!(
            engine
                .evaluate(
                    "account",
                    AchievementEvent::Runtime {
                        game_id: &game,
                        event: &playable
                    }
                )
                .unwrap()
                .is_empty()
        );
        assert_eq!(engine.pending_sync(10).unwrap().len(), 1);
    }

    #[test]
    fn animal_crossing_and_ryan_moment_have_deliberate_triggers() {
        let temp = tempfile::tempdir().unwrap();
        let mut engine = AchievementEngine::open(
            &temp.path().join("achievements.sqlite3"),
            OverlayQueue::default(),
        )
        .unwrap();
        let acnh = GameId::new("animal-crossing-new-horizons").unwrap();
        assert_eq!(
            engine
                .evaluate(
                    "account",
                    AchievementEvent::GameLaunched {
                        game_id: &acnh,
                        at_unix_ms: 1
                    }
                )
                .unwrap()[0]
                .achievement_id,
            "that-is-not-zelda"
        );
        assert!(
            engine
                .evaluate(
                    "account",
                    AchievementEvent::ClassifiedRyanMoment {
                        code: "FICSIT-0009",
                        at_unix_ms: 2
                    }
                )
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            engine
                .evaluate(
                    "account",
                    AchievementEvent::ClassifiedRyanMoment {
                        code: "FICSIT-0010",
                        at_unix_ms: 3
                    }
                )
                .unwrap()[0]
                .achievement_id,
            "ryan-moment"
        );
    }

    #[test]
    fn treaty_data_and_satisfactory_events_have_explicit_triggers() {
        let temp = tempfile::tempdir().unwrap();
        let mut engine = AchievementEngine::open(
            &temp.path().join("achievements.sqlite3"),
            OverlayQueue::default(),
        )
        .unwrap();
        let data = engine
            .evaluate(
                "account",
                AchievementEvent::GameDataValidated { at_unix_ms: 1 },
            )
            .unwrap();
        assert_eq!(data[0].achievement_id, "legally-supplied-bits");
        let qualifying = engine
            .evaluate(
                "account",
                AchievementEvent::SatisfactorySessionQualified { at_unix_ms: 2 },
            )
            .unwrap();
        assert_eq!(
            qualifying
                .iter()
                .map(|unlock| unlock.achievement_id.as_str())
                .collect::<Vec<_>>(),
            vec!["ficsit-employee-onboarding", "article-ii-enjoyer"]
        );
        let hydration = engine
            .evaluate(
                "account",
                AchievementEvent::HydrationAcknowledged {
                    acknowledgements: 5,
                    at_unix_ms: 3,
                },
            )
            .unwrap();
        assert_eq!(hydration[1].achievement_id, "pipeline-operational");
    }
}
