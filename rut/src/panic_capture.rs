use crate::{SourceLocation, TestResult};
use std::any::Any;
use std::backtrace::Backtrace;
use std::cell::RefCell;
use std::future::Future;
use std::panic::{AssertUnwindSafe, PanicHookInfo};
use std::pin::Pin;
use std::sync::{Arc, Mutex, Once};
use std::task::{Context, Poll};

#[derive(Debug)]
struct PanicRecord {
    message: String,
    location: Option<SourceLocation>,
    backtrace: String,
}

type CaptureSlot = Arc<Mutex<Option<PanicRecord>>>;

thread_local! {
    static ACTIVE_CAPTURES: RefCell<Vec<CaptureSlot>> = const { RefCell::new(Vec::new()) };
}

static INSTALL_HOOK: Once = Once::new();

pub(crate) async fn catch_test_panic<F>(future: F) -> Result<F::Output, TestResult>
where
    F: Future,
{
    CatchTestPanic {
        future: Some(Box::pin(future)),
        capture: Arc::new(Mutex::new(None)),
    }
    .await
}

struct CatchTestPanic<F> {
    future: Option<Pin<Box<F>>>,
    capture: CaptureSlot,
}

impl<F> Future for CatchTestPanic<F>
where
    F: Future,
{
    type Output = Result<F::Output, TestResult>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        install_hook();
        let this = self.get_mut();
        ACTIVE_CAPTURES.with(|captures| captures.borrow_mut().push(this.capture.clone()));
        let poll = std::panic::catch_unwind(AssertUnwindSafe(|| {
            this.future
                .as_mut()
                .expect("panic-catching future polled after completion")
                .as_mut()
                .poll(context)
        }));
        ACTIVE_CAPTURES.with(|captures| {
            captures.borrow_mut().pop();
        });

        match poll {
            Ok(Poll::Ready(output)) => {
                this.future = None;
                Poll::Ready(Ok(output))
            }
            Ok(Poll::Pending) => Poll::Pending,
            Err(payload) => {
                this.future = None;
                let record = this
                    .capture
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .take()
                    .unwrap_or_else(|| PanicRecord {
                        message: panic_payload(payload.as_ref()),
                        location: None,
                        backtrace: Backtrace::force_capture().to_string(),
                    });
                Poll::Ready(Err(panic_result(record)))
            }
        }
    }
}

fn install_hook() {
    INSTALL_HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !capture_panic(info) {
                previous(info);
            }
        }));
    });
}

fn capture_panic(info: &PanicHookInfo<'_>) -> bool {
    ACTIVE_CAPTURES.with(|captures| {
        let capture = captures.borrow().last().cloned();
        let Some(capture) = capture else {
            return false;
        };
        let location = info.location().map(|location| {
            SourceLocation::new(location.file(), location.line(), location.column())
        });
        let mut slot = capture
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *slot = Some(PanicRecord {
            message: panic_payload(info.payload()),
            location,
            backtrace: Backtrace::force_capture().to_string(),
        });
        true
    })
}

fn panic_payload(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "panic with non-string payload".to_string()
    }
}

fn panic_result(record: PanicRecord) -> TestResult {
    let mut result = TestResult::failed(format!(
        "panic: {}\nstack backtrace:\n{}",
        record.message, record.backtrace
    ));
    result.failure_location = record.location;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn captures_panic_origin_and_backtrace() {
        let line = line!() + 2;
        let result = catch_test_panic(async move {
            panic!("boom");
        })
        .await
        .unwrap_err();

        let location = result.failure_location.unwrap();
        assert_eq!(location.file, file!());
        assert_eq!(location.line, line);
        assert!(result.message.unwrap().contains("stack backtrace:"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn keeps_concurrent_panic_payloads_separate() {
        let first = tokio::spawn(catch_test_panic(async { panic!("first panic") }));
        let second = tokio::spawn(catch_test_panic(async { panic!("second panic") }));

        let first = first.await.unwrap().unwrap_err().message.unwrap();
        let second = second.await.unwrap().unwrap_err().message.unwrap();
        assert!(first.contains("first panic"));
        assert!(!first.contains("second panic"));
        assert!(second.contains("second panic"));
        assert!(!second.contains("first panic"));
    }
}
