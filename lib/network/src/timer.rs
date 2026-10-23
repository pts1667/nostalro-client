//! The two timers the network loop needs: tokio's on native, and the same
//! shapes over `setTimeout` in a browser, where there is no tokio runtime.

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use tokio::time::{interval, sleep};

#[cfg(target_arch = "wasm32")]
pub(crate) use web::{interval, sleep};

#[cfg(target_arch = "wasm32")]
mod web {
    use std::time::Duration;

    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_time::Instant;

    pub(crate) async fn sleep(duration: Duration) {
        let ms = duration.as_millis().min(i32::MAX as u128) as i32;
        let promise = js_sys::Promise::new(&mut |resolve, _reject| {
            let Some(window) = web_sys::window() else { return };
            let _ = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(resolve.unchecked_ref(), ms);
        });
        let _ = JsFuture::from(promise).await;
    }

    /// Ticks every `period`, a missed tick firing as soon as it is polled, like
    /// tokio's default. Dropping a `tick` that has not fired keeps its deadline.
    pub(crate) struct Interval {
        period: Duration,
        next: Instant,
    }

    pub(crate) fn interval(period: Duration) -> Interval {
        Interval { period, next: Instant::now() }
    }

    impl Interval {
        pub(crate) fn reset(&mut self) {
            self.next = Instant::now() + self.period;
        }

        pub(crate) async fn tick(&mut self) {
            let wait = self.next.saturating_duration_since(Instant::now());
            if !wait.is_zero() {
                sleep(wait).await;
            }
            self.next += self.period;
        }
    }
}
