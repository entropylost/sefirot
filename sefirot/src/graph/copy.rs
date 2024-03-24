use std::sync::Arc;

use parking_lot::lock_api::{ArcMutexGuard, ArcRwLockWriteGuard};
use parking_lot::{Mutex, RawMutex, RawRwLock, RwLock};

use super::*;

pub trait SliceGuard<T>: Send + 'static {
    fn get_slice_mut(&mut self) -> &mut [T];
}

pub trait LockSlice<T> {
    type Guard: SliceGuard<T>;
    fn lock_arc(&self) -> Self::Guard;
}
impl<T: Send + 'static> SliceGuard<T> for ArcMutexGuard<RawMutex, Vec<T>> {
    fn get_slice_mut(&mut self) -> &mut [T] {
        self
    }
}
impl<T: Send + 'static> LockSlice<T> for Arc<Mutex<Vec<T>>> {
    type Guard = ArcMutexGuard<RawMutex, Vec<T>>;
    fn lock_arc(&self) -> Self::Guard {
        self.lock_arc()
    }
}
impl<T: Send + Sync + 'static> SliceGuard<T> for ArcRwLockWriteGuard<RawRwLock, Vec<T>> {
    fn get_slice_mut(&mut self) -> &mut [T] {
        self
    }
}
impl<T: Send + Sync + 'static> LockSlice<T> for Arc<RwLock<Vec<T>>> {
    type Guard = ArcRwLockWriteGuard<RawRwLock, Vec<T>>;
    fn lock_arc(&self) -> Self::Guard {
        self.write_arc()
    }
}
impl<const N: usize, T: Send + 'static> SliceGuard<T> for ArcMutexGuard<RawMutex, [T; N]> {
    fn get_slice_mut(&mut self) -> &mut [T] {
        &mut self[..]
    }
}
impl<const N: usize, T: Send + 'static> LockSlice<T> for Arc<Mutex<[T; N]>> {
    type Guard = ArcMutexGuard<RawMutex, [T; N]>;
    fn lock_arc(&self) -> Self::Guard {
        self.lock_arc()
    }
}
impl<const N: usize, T: Send + Sync + 'static> SliceGuard<T>
    for ArcRwLockWriteGuard<RawRwLock, [T; N]>
{
    fn get_slice_mut(&mut self) -> &mut [T] {
        &mut self[..]
    }
}
impl<const N: usize, T: Send + Sync + 'static> LockSlice<T> for Arc<RwLock<[T; N]>> {
    type Guard = ArcRwLockWriteGuard<RawRwLock, [T; N]>;
    fn lock_arc(&self) -> Self::Guard {
        self.write_arc()
    }
}

pub trait CopyExt<T: Value + Send> {
    fn copy_to_shared(&self, dst: &impl LockSlice<T>) -> NodeConfigs<'static>;
    fn copy_from_shared(&self, src: &impl LockSlice<T>) -> NodeConfigs<'static>;
    fn copy_from_vec(&self, src: Vec<T>) -> NodeConfigs<'static> {
        let src = Arc::new(Mutex::new(src));
        self.copy_from_shared(&src)
    }
}
impl<T: Value + Send> CopyExt<T> for BufferView<T> {
    fn copy_to_shared(&self, dst: &impl LockSlice<T>) -> NodeConfigs<'static> {
        let src = self.clone();
        let mut guard = dst.lock_arc();
        let dst =
            unsafe { std::mem::transmute::<&mut [T], &'static mut [T]>(guard.get_slice_mut()) };
        src.copy_to_async(dst).release(guard)
    }
    fn copy_from_shared(&self, src: &impl LockSlice<T>) -> NodeConfigs<'static> {
        let dst = self.clone();
        let mut guard = src.lock_arc();
        let src =
            unsafe { std::mem::transmute::<&mut [T], &'static mut [T]>(guard.get_slice_mut()) };
        dst.copy_from_async(src).release(guard)
    }
}
impl<T: StorageTexel<U> + Value + Send, U: IoTexel> CopyExt<T> for Tex2dView<U> {
    fn copy_to_shared(&self, dst: &impl LockSlice<T>) -> NodeConfigs<'static> {
        let src = self.clone();
        let mut guard = dst.lock_arc();
        let dst =
            unsafe { std::mem::transmute::<&mut [T], &'static mut [T]>(guard.get_slice_mut()) };
        src.copy_to_async(dst).release(guard)
    }
    fn copy_from_shared(&self, src: &impl LockSlice<T>) -> NodeConfigs<'static> {
        let dst = self.clone();
        let mut guard = src.lock_arc();
        let src =
            unsafe { std::mem::transmute::<&mut [T], &'static mut [T]>(guard.get_slice_mut()) };
        dst.copy_from_async(src).release(guard)
    }
}
impl<T: StorageTexel<U> + Value + Send, U: IoTexel> CopyExt<T> for Tex3dView<U> {
    fn copy_to_shared(&self, dst: &impl LockSlice<T>) -> NodeConfigs<'static> {
        let src = self.clone();
        let mut guard = dst.lock_arc();
        let dst =
            unsafe { std::mem::transmute::<&mut [T], &'static mut [T]>(guard.get_slice_mut()) };
        src.copy_to_async(dst).release(guard)
    }
    fn copy_from_shared(&self, src: &impl LockSlice<T>) -> NodeConfigs<'static> {
        let dst = self.clone();
        let mut guard = src.lock_arc();
        let src =
            unsafe { std::mem::transmute::<&mut [T], &'static mut [T]>(guard.get_slice_mut()) };
        dst.copy_from_async(src).release(guard)
    }
}
impl<T: Value + Send> CopyExt<T> for Buffer<T> {
    fn copy_to_shared(&self, dst: &impl LockSlice<T>) -> NodeConfigs<'static> {
        self.view(..).copy_to_shared(dst)
    }
    fn copy_from_shared(&self, src: &impl LockSlice<T>) -> NodeConfigs<'static> {
        self.view(..).copy_from_shared(src)
    }
}
impl<T: StorageTexel<U> + Value + Send, U: IoTexel> CopyExt<T> for Tex2d<U> {
    fn copy_to_shared(&self, dst: &impl LockSlice<T>) -> NodeConfigs<'static> {
        self.view(0).copy_to_shared(dst)
    }
    fn copy_from_shared(&self, src: &impl LockSlice<T>) -> NodeConfigs<'static> {
        self.view(0).copy_from_shared(src)
    }
}
impl<T: StorageTexel<U> + Value + Send, U: IoTexel> CopyExt<T> for Tex3d<U> {
    fn copy_to_shared(&self, dst: &impl LockSlice<T>) -> NodeConfigs<'static> {
        self.view(0).copy_to_shared(dst)
    }
    fn copy_from_shared(&self, src: &impl LockSlice<T>) -> NodeConfigs<'static> {
        self.view(0).copy_from_shared(src)
    }
}
