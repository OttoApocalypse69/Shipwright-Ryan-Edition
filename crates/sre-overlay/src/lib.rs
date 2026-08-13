use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OverlayKind {
    Achievement,
    HydrationReminder,
    TreatyWarning,
    RuntimeNotification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayNotification {
    pub id: String,
    pub kind: OverlayKind,
    pub title: String,
    pub message: String,
    pub created_at_unix_ms: u64,
    pub display_ms: u64,
}

#[derive(Clone, Default)]
pub struct OverlayQueue {
    inner: Arc<Mutex<VecDeque<OverlayNotification>>>,
}

impl OverlayQueue {
    pub fn push(&self, notification: OverlayNotification) -> Result<(), String> {
        let mut queue = self
            .inner
            .lock()
            .map_err(|_| "overlay queue is unavailable")?;
        if !queue.iter().any(|value| value.id == notification.id) {
            queue.push_back(notification);
        }
        Ok(())
    }

    pub fn pop(&self) -> Result<Option<OverlayNotification>, String> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| "overlay queue is unavailable")?
            .pop_front())
    }

    pub fn len(&self) -> Result<usize, String> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| "overlay queue is unavailable")?
            .len())
    }

    pub fn is_empty(&self) -> Result<bool, String> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| "overlay queue is unavailable")?
            .is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queue_preserves_order_and_deduplicates() {
        let queue = OverlayQueue::default();
        let value = OverlayNotification {
            id: "one".to_owned(),
            kind: OverlayKind::Achievement,
            title: "Title".to_owned(),
            message: "Message".to_owned(),
            created_at_unix_ms: 0,
            display_ms: 5_000,
        };
        queue.push(value.clone()).unwrap();
        queue.push(value.clone()).unwrap();
        assert_eq!(queue.len().unwrap(), 1);
        assert_eq!(queue.pop().unwrap(), Some(value));
        assert!(queue.is_empty().unwrap());
    }
}
