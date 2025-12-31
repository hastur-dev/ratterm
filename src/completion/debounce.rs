//! Debounce state for completion requests.
//!
//! Implements a debounce mechanism that waits for a configurable delay
//! before triggering completions, with support for cancellation on new input.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::Notify;

/// Default debounce delay in milliseconds.
pub const DEFAULT_DEBOUNCE_MS: u64 = 300;

/// Maximum debounce delay in milliseconds.
pub const MAX_DEBOUNCE_MS: u64 = 2000;

/// Debounce state for managing completion request timing.
///
/// When a completion is triggered, it waits for the debounce delay
/// before actually requesting completions. If another trigger occurs
/// during the delay, the previous request is cancelled and the timer
/// resets.
#[derive(Debug)]
pub struct DebounceState {
    /// Debounce delay duration.
    delay: Duration,

    /// Current request ID (incremented on each trigger).
    current_id: AtomicU64,

    /// Notify channel for cancellation.
    cancel_notify: Arc<Notify>,

    /// Last trigger timestamp (for debugging/metrics).
    last_trigger: std::sync::Mutex<Option<Instant>>,
}

impl DebounceState {
    /// Creates a new debounce state with the given delay.
    ///
    /// # Panics
    ///
    /// Panics if delay is zero or greater than `MAX_DEBOUNCE_MS`.
    #[must_use]
    pub fn new(delay: Duration) -> Self {
        assert!(!delay.is_zero(), "debounce delay must be positive");
        assert!(
            delay.as_millis() <= u128::from(MAX_DEBOUNCE_MS),
            "debounce delay must be <= {} ms",
            MAX_DEBOUNCE_MS
        );

        Self {
            delay,
            current_id: AtomicU64::new(0),
            cancel_notify: Arc::new(Notify::new()),
            last_trigger: std::sync::Mutex::new(None),
        }
    }

    /// Creates a new debounce state with the default delay (300ms).
    #[must_use]
    pub fn with_default_delay() -> Self {
        Self::new(Duration::from_millis(DEFAULT_DEBOUNCE_MS))
    }

    /// Triggers a new debounced request.
    ///
    /// Returns the request ID that can be used to check if the
    /// request is still valid after the delay.
    pub fn trigger(&self) -> u64 {
        // Increment the ID and notify any waiters to cancel
        let id = self.current_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.cancel_notify.notify_waiters();

        // Update last trigger time
        if let Ok(mut last) = self.last_trigger.lock() {
            *last = Some(Instant::now());
        }

        id
    }

    /// Waits for the debounce delay, returning the request ID if
    /// not cancelled.
    ///
    /// Returns `None` if a new trigger occurred during the wait,
    /// indicating this request was cancelled.
    pub async fn wait(&self, request_id: u64) -> Option<u64> {
        let cancel_notify = Arc::clone(&self.cancel_notify);

        // Race between delay completing and cancellation
        tokio::select! {
            () = tokio::time::sleep(self.delay) => {
                // Check if our request is still the current one
                let current = self.current_id.load(Ordering::SeqCst);
                if current == request_id {
                    Some(request_id)
                } else {
                    None
                }
            }
            () = cancel_notify.notified() => {
                // Cancelled by a newer request
                None
            }
        }
    }

    /// Cancels any pending debounced request.
    pub fn cancel(&self) {
        self.current_id.fetch_add(1, Ordering::SeqCst);
        self.cancel_notify.notify_waiters();
    }

    /// Returns the current request ID.
    #[must_use]
    pub fn current_id(&self) -> u64 {
        self.current_id.load(Ordering::SeqCst)
    }

    /// Returns whether a request ID is still valid (not cancelled).
    #[must_use]
    pub fn is_valid(&self, request_id: u64) -> bool {
        self.current_id.load(Ordering::SeqCst) == request_id
    }

    /// Returns the debounce delay.
    #[must_use]
    pub const fn delay(&self) -> Duration {
        self.delay
    }
}

impl Default for DebounceState {
    fn default() -> Self {
        Self::with_default_delay()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_debounce_creation() {
        let debounce = DebounceState::new(Duration::from_millis(100));
        assert_eq!(debounce.delay().as_millis(), 100);
        assert_eq!(debounce.current_id(), 0);
    }

    #[test]
    fn test_debounce_trigger_increments_id() {
        let debounce = DebounceState::new(Duration::from_millis(100));

        let id1 = debounce.trigger();
        assert_eq!(id1, 1);

        let id2 = debounce.trigger();
        assert_eq!(id2, 2);

        let id3 = debounce.trigger();
        assert_eq!(id3, 3);
    }

    #[test]
    fn test_debounce_is_valid() {
        let debounce = DebounceState::new(Duration::from_millis(100));

        let id1 = debounce.trigger();
        assert!(debounce.is_valid(id1));

        let id2 = debounce.trigger();
        assert!(!debounce.is_valid(id1));
        assert!(debounce.is_valid(id2));
    }

    #[test]
    fn test_debounce_cancel() {
        let debounce = DebounceState::new(Duration::from_millis(100));

        let id = debounce.trigger();
        assert!(debounce.is_valid(id));

        debounce.cancel();
        assert!(!debounce.is_valid(id));
    }

    #[tokio::test]
    async fn test_debounce_wait_completes() {
        let debounce = DebounceState::new(Duration::from_millis(10));

        let id = debounce.trigger();
        let result = debounce.wait(id).await;

        assert_eq!(result, Some(id));
    }

    #[tokio::test]
    async fn test_debounce_wait_cancelled() {
        let debounce = Arc::new(DebounceState::new(Duration::from_millis(100)));

        let id = debounce.trigger();

        let debounce_clone = Arc::clone(&debounce);
        let handle = tokio::spawn(async move { debounce_clone.wait(id).await });

        // Trigger a new request to cancel the previous one
        tokio::time::sleep(Duration::from_millis(10)).await;
        debounce.trigger();

        let result = handle.await.unwrap();
        assert_eq!(result, None);
    }

    #[test]
    #[should_panic(expected = "debounce delay must be positive")]
    fn test_debounce_zero_delay_panics() {
        let _ = DebounceState::new(Duration::ZERO);
    }

    #[test]
    #[should_panic(expected = "debounce delay must be <=")]
    fn test_debounce_too_long_delay_panics() {
        let _ = DebounceState::new(Duration::from_millis(MAX_DEBOUNCE_MS + 1));
    }
}
