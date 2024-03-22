use std::sync::Arc;

use generational_arena::{Arena, Index};
use parking_lot::Mutex;

use super::buffer::{HandledBuffer, HandledTex2d};
use crate::internal_prelude::*;

pub struct SwapHandle(Arc<dyn CanSwap>);
impl SwapHandle {
    pub fn swap(&self) {
        self.0.swap();
    }
}

pub trait CanSwap {
    fn swap(&self);
}

struct Swap<T> {
    internal: Mutex<SwapInternal<T>>,
}

impl<T> CanSwap for Swap<T> {
    fn swap(&self) {
        let mut internal_guard = self.internal.lock();
        let SwapInternal { alpha, beta } = &mut *internal_guard;
        std::mem::swap(alpha, beta);
    }
}

struct SwapInternal<T> {
    alpha: T,
    beta: T,
}

pub struct SwapBufferMapping(Arc<Mutex<Swap>>);
