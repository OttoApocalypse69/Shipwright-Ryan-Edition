//! Human-safe FTEP Accord evaluation. Policy records cooperation but never
//! performs invasive enforcement or treats voluntary play as contractual debt.

use serde::{Deserialize, Serialize};

pub const ACCORD_JSON: &str = include_str!("../../../packages/treaty/FICSIT-ACCORD-0001.v3.json");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Accord {
    pub treaty_id: String,
    pub version: String,
    pub title: String,
    pub schedule: SchedulePolicy,
    pub articles: Vec<AccordArticle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchedulePolicy {
    pub qualifying_minutes: u16,
    pub expected_min_minutes: u16,
    pub expected_max_minutes: u16,
    pub daily_recognition_cap_minutes: u16,
    pub real_life_overrides: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccordArticle {
    pub number: u8,
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComplianceState {
    Compliant,
    Warning,
    MaterialBreach,
    Remediation,
    Restored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScopeRequestState {
    Requested,
    UnderReview,
    Accepted,
    InProgress,
    Supported,
    Rejected,
    Deferred,
}

pub fn load_accord() -> Result<Accord, String> {
    let accord: Accord = serde_json::from_str(ACCORD_JSON).map_err(|error| error.to_string())?;
    if accord.articles.len() != 17
        || !accord
            .articles
            .iter()
            .enumerate()
            .all(|(index, article)| article.number as usize == index + 1)
    {
        return Err("Accord must define Articles I through XVII in order".to_owned());
    }
    if accord.schedule.qualifying_minutes < 30
        || accord.schedule.expected_min_minutes < 30
        || accord.schedule.expected_max_minutes > 120
        || accord.schedule.daily_recognition_cap_minutes > 240
        || !accord.schedule.real_life_overrides
    {
        return Err("Accord scheduling policy is unreasonable".to_owned());
    }
    Ok(accord)
}

pub fn recognized_minutes(policy: &SchedulePolicy, voluntary_minutes: u16) -> u16 {
    voluntary_minutes.min(policy.daily_recognition_cap_minutes)
}

pub fn qualifies(policy: &SchedulePolicy, voluntary_minutes: u16) -> bool {
    recognized_minutes(policy, voluntary_minutes) >= policy.qualifying_minutes
}

pub fn transition(state: ComplianceState, remediation_completed: bool) -> ComplianceState {
    match (state, remediation_completed) {
        (ComplianceState::MaterialBreach, true) => ComplianceState::Remediation,
        (ComplianceState::Remediation, true) => ComplianceState::Restored,
        (ComplianceState::Restored, _) => ComplianceState::Compliant,
        (other, _) => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accord_is_complete_and_rejects_abusive_scheduling() {
        let accord = load_accord().unwrap();
        assert_eq!(accord.articles.len(), 17);
        assert!(!qualifies(&accord.schedule, 29));
        assert!(qualifies(&accord.schedule, 30));
        assert_eq!(recognized_minutes(&accord.schedule, 1_080), 240);
    }

    #[test]
    fn remediation_restores_compliance_without_destructive_action() {
        assert_eq!(
            transition(ComplianceState::MaterialBreach, true),
            ComplianceState::Remediation
        );
        assert_eq!(
            transition(ComplianceState::Remediation, true),
            ComplianceState::Restored
        );
        assert_eq!(
            transition(ComplianceState::Restored, false),
            ComplianceState::Compliant
        );
    }
}
