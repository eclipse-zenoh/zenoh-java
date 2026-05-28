/// Calls a no-arg closure on drop. Lets a long-lived callback closure
/// signal teardown by capturing one of these alongside its main
/// callback — when the outer holder is dropped, this fires.
pub(crate) struct OnceDrop<F: Fn() + Send + Sync + 'static> {
    f: Option<F>,
}

impl<F: Fn() + Send + Sync + 'static> OnceDrop<F> {
    pub(crate) fn new(f: F) -> Self {
        Self { f: Some(f) }
    }
}

impl<F: Fn() + Send + Sync + 'static> Drop for OnceDrop<F> {
    fn drop(&mut self) {
        if let Some(f) = self.f.take() {
            f();
        }
    }
}
