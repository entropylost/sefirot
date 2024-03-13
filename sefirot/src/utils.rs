/// A struct that runs a given function upon drop.
#[derive(Debug, Clone)]
pub struct FnRelease<F: FnOnce() + 'static>(Option<F>);
impl<F: FnOnce() + 'static> FnRelease<F> {
    pub fn new(f: F) -> Self {
        Self(Some(f))
    }
}
impl<F: FnOnce() + 'static> Drop for FnRelease<F> {
    fn drop(&mut self) {
        self.0.take().unwrap()();
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Paradox {}
