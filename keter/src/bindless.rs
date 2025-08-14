use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::LazyLock;

use luisa_compute::prelude::*;
use luisa_compute::runtime::{
    AsKernelArg, KernelArg, KernelArgEncoder, KernelBuilder, KernelParameter,
};

pub static BINDLESS: LazyLock<Bindless> = LazyLock::new(|| {
    Bindless::new(
        crate::DEVICE.create_bindless_array(65536),
        crate::DEVICE.clone(),
    )
});

#[derive(Debug)]
pub struct Tex2dHandle<T: IoTexel> {
    index: u32,
    pub texture: Tex2d<T>,
    pub sampler: Sampler,
}
#[derive(Debug)]
pub struct Tex3dHandle<T: IoTexel> {
    index: u32,
    pub texture: Tex3d<T>,
    pub sampler: Sampler,
}
#[derive(Debug)]
pub struct BufferHandle<T: Value> {
    index: u32,
    pub buffer: Buffer<T>,
}
impl<T: IoTexel> Deref for Tex2dHandle<T> {
    type Target = Tex2d<T>;
    fn deref(&self) -> &Self::Target {
        &self.texture
    }
}
impl<T: IoTexel> Deref for Tex3dHandle<T> {
    type Target = Tex3d<T>;
    fn deref(&self) -> &Self::Target {
        &self.texture
    }
}
impl<T: Value> Deref for BufferHandle<T> {
    type Target = Buffer<T>;
    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

#[derive(Clone)]
pub struct Tex2dHandleVar<T: IoTexel> {
    internal: BindlessTex2dVar,
    // index: Expr<u32>,
    _marker: PhantomData<T>,
}
#[derive(Clone)]
pub struct Tex3dHandleVar<T: IoTexel> {
    internal: BindlessTex3dVar,
    // index: Expr<u32>,
    _marker: PhantomData<T>,
}
#[derive(Clone)]
pub struct BufferHandleVar<T: Value> {
    internal: BindlessBufferVar<T>,
    // index: Expr<u32>,
    // _marker: PhantomData<T>,
}
impl<T: IoTexel> Deref for Tex2dHandleVar<T> {
    type Target = BindlessTex2dVar;
    fn deref(&self) -> &Self::Target {
        &self.internal
    }
}
impl<T: IoTexel> Deref for Tex3dHandleVar<T> {
    type Target = BindlessTex3dVar;
    fn deref(&self) -> &Self::Target {
        &self.internal
    }
}
impl<T: Value> Deref for BufferHandleVar<T> {
    type Target = BindlessBufferVar<T>;
    fn deref(&self) -> &Self::Target {
        &self.internal
    }
}

impl<T: IoTexel> KernelArg for Tex2dHandle<T> {
    type Parameter = Tex2dHandleVar<T>;
    fn encode(&self, encoder: &mut KernelArgEncoder) {
        self.index.encode(encoder);
    }
}
impl<T: IoTexel> KernelParameter for Tex2dHandleVar<T> {
    type Arg = Tex2dHandle<T>;
    fn def_param(builder: &mut KernelBuilder) -> Self {
        Self {
            internal: BINDLESS.var().array.tex2d(Expr::<u32>::def_param(builder)),
            _marker: PhantomData,
        }
    }
}
impl<T: IoTexel> AsKernelArg for Tex2dHandle<T> {
    type Output = Tex2dHandle<T>;
}
impl<T: IoTexel> KernelArg for Tex3dHandle<T> {
    type Parameter = Tex3dHandleVar<T>;
    fn encode(&self, encoder: &mut KernelArgEncoder) {
        self.index.encode(encoder);
    }
}
impl<T: IoTexel> KernelParameter for Tex3dHandleVar<T> {
    type Arg = Tex3dHandle<T>;
    fn def_param(builder: &mut KernelBuilder) -> Self {
        Self {
            internal: BINDLESS.var().array.tex3d(Expr::<u32>::def_param(builder)),
            _marker: PhantomData,
        }
    }
}
impl<T: IoTexel> AsKernelArg for Tex3dHandle<T> {
    type Output = Tex3dHandle<T>;
}
impl<T: Value> KernelArg for BufferHandle<T> {
    type Parameter = BufferHandleVar<T>;
    fn encode(&self, encoder: &mut KernelArgEncoder) {
        self.index.encode(encoder);
    }
}
impl<T: Value> KernelParameter for BufferHandleVar<T> {
    type Arg = BufferHandle<T>;
    fn def_param(builder: &mut KernelBuilder) -> Self {
        Self {
            internal: BINDLESS.var().array.buffer(Expr::<u32>::def_param(builder)),
            // _marker: PhantomData,
        }
    }
}
impl<T: Value> AsKernelArg for BufferHandle<T> {
    type Output = BufferHandle<T>;
}

pub struct Bindless {
    pub array: BindlessArray,
    next_tex2d: AtomicU32,
    next_tex3d: AtomicU32,
    next_buffer: AtomicU32,
    needs_update: AtomicBool,
    device: Device,
}
pub struct BindlessVar {
    pub array: BindlessArrayVar,
}
impl Bindless {
    pub fn new(array: BindlessArray, array_device: Device) -> Self {
        Self {
            array,
            next_tex2d: 0.into(),
            next_tex3d: 0.into(),
            next_buffer: 0.into(),
            needs_update: false.into(),
            device: array_device,
        }
    }
    pub fn push_tex2d<T: IoTexel>(&self, texture: Tex2d<T>, sampler: Sampler) -> Tex2dHandle<T> {
        let index = self.next_tex2d.fetch_add(1, Ordering::Relaxed);
        self.needs_update.store(true, Ordering::Relaxed);
        self.array
            .emplace_tex2d_async(index as usize, &texture, sampler);
        Tex2dHandle {
            index,
            texture,
            sampler,
        }
    }
    pub fn push_tex3d<T: IoTexel>(&self, texture: Tex3d<T>, sampler: Sampler) -> Tex3dHandle<T> {
        let index = self.next_tex3d.fetch_add(1, Ordering::Relaxed);
        self.needs_update.store(true, Ordering::Relaxed);
        self.array
            .emplace_tex3d_async(index as usize, &texture, sampler);
        Tex3dHandle {
            index,
            texture,
            sampler,
        }
    }
    pub fn push_buffer<T: Value>(&self, buffer: Buffer<T>) -> BufferHandle<T> {
        let index = self.next_buffer.fetch_add(1, Ordering::Relaxed);
        self.needs_update.store(true, Ordering::Relaxed);
        self.array.emplace_buffer_async(index as usize, &buffer);
        BufferHandle { index, buffer }
    }
    pub fn create_tex2d<T: IoTexel>(
        &self,
        storage: PixelStorage,
        width: u32,
        height: u32,
        mips: u32,
        sampler: Sampler,
    ) -> Tex2dHandle<T> {
        let texture = self.device.create_tex2d(storage, width, height, mips);
        self.push_tex2d(texture, sampler)
    }
    pub fn create_tex3d<T: IoTexel>(
        &self,
        storage: PixelStorage,
        width: u32,
        height: u32,
        depth: u32,
        mips: u32,
        sampler: Sampler,
    ) -> Tex3dHandle<T> {
        let texture = self
            .device
            .create_tex3d(storage, width, height, depth, mips);
        self.push_tex3d(texture, sampler)
    }
    pub fn create_buffer<T: Value>(&self, count: usize) -> BufferHandle<T> {
        let buffer = self.device.create_buffer(count);
        self.push_buffer(buffer)
    }
    pub fn create_buffer_from_slice<T: Value>(&self, slice: &[T]) -> BufferHandle<T> {
        let buffer = self.device.create_buffer_from_slice(slice);
        self.push_buffer(buffer)
    }
    pub fn create_buffer_from_fn<T: Value, F: FnMut(usize) -> T>(
        &self,
        count: usize,
        f: F,
    ) -> BufferHandle<T> {
        let buffer = self.device.create_buffer_from_fn(count, f);
        self.push_buffer(buffer)
    }
    pub fn update(&self) {
        self.array.update();
    }
    pub fn var(&self) -> BindlessVar {
        BindlessVar {
            array: self.array.var(),
        }
    }
}
impl KernelArg for Bindless {
    type Parameter = BindlessVar;
    fn encode(&self, encoder: &mut KernelArgEncoder) {
        debug_assert!(
            !self.needs_update.load(Ordering::Relaxed),
            "Bindless array needs update before encoding"
        );
        self.array.encode(encoder);
    }
}
impl KernelParameter for BindlessVar {
    type Arg = Bindless;
    fn def_param(builder: &mut KernelBuilder) -> Self {
        Self {
            array: BindlessArrayVar::def_param(builder),
        }
    }
}
impl AsKernelArg for Bindless {
    type Output = Bindless;
}
