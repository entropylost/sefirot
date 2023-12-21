use luisa::lang::types::vector::{Vec2, Vec3};

use super::array::IntoBuffer;
use super::*;

#[derive(Debug, Clone)]
pub struct Slice<V: Any> {
    size: u32,
    check_bounds: bool,
    access: Arc<dyn SliceAccessor<V>>,
}
impl<V: Any> Slice<V> {
    pub fn size(&self) -> u32 {
        self.size
    }
    pub fn check_bounds(&self) -> bool {
        self.check_bounds
    }
    #[tracked]
    pub fn read(&self, index: Expr<u32>) -> V {
        if self.check_bounds {
            let i = index < self.size;
            lc_assert!(i);
        }
        self.access.read(index)
    }
    #[tracked]
    pub fn write(&self, index: Expr<u32>, value: V) {
        if !self.can_write() {
            panic!("Cannot write to slice without write access.");
        };
        if self.check_bounds {
            let i = index < self.size;
            lc_assert!(i);
        }
        self.access.write(index, value)
    }
    pub fn can_write(&self) -> bool {
        self.access.can_write()
    }
}

pub trait SliceAccessor<V: Any> {
    fn read(&self, index: Expr<u32>) -> V;
    fn write(&self, index: Expr<u32>, value: V);
    fn can_write(&self) -> bool;
    fn type_name(&self) -> String {
        pretty_type_name::<Self>()
    }
}
impl<V: Any> Debug for dyn SliceAccessor<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.type_name())
    }
}

pub struct BufferSliceAccessor<V: Value> {
    buffer: BufferView<V>,
    offset: Expr<u32>,
}

impl<V: Value> SliceAccessor<Expr<V>> for BufferSliceAccessor<V> {
    #[tracked]
    fn read(&self, index: Expr<u32>) -> Expr<V> {
        self.buffer.var().read(self.offset + index)
    }
    #[tracked]
    fn write(&self, index: Expr<u32>, value: Expr<V>) {
        self.buffer.var().write(self.offset + index, value);
    }
    fn can_write(&self) -> bool {
        true
    }
}

pub struct Tex2dSliceAccessor<V: IoTexel> {
    texture: Tex2dView<V>,
    index: Expr<u32>,
}

impl<V: IoTexel> SliceAccessor<Expr<V>> for Tex2dSliceAccessor<V> {
    #[tracked]
    fn read(&self, index: Expr<u32>) -> Expr<V> {
        self.texture.var().read(Vec2::expr(self.index, index))
    }
    #[tracked]
    fn write(&self, index: Expr<u32>, value: Expr<V>) {
        self.texture
            .var()
            .write(Vec2::expr(self.index, index), value);
    }
    fn can_write(&self) -> bool {
        true
    }
}

pub struct Tex3dSliceAccessor<V: IoTexel> {
    texture: Tex3dView<V>,
    index: Expr<Vec2<u32>>,
}

impl<V: IoTexel> SliceAccessor<Expr<V>> for Tex3dSliceAccessor<V> {
    #[tracked]
    fn read(&self, index: Expr<u32>) -> Expr<V> {
        self.texture
            .var()
            .read(Vec3::expr(self.index.x, self.index.y, index))
    }
    #[tracked]
    fn write(&self, index: Expr<u32>, value: Expr<V>) {
        self.texture
            .var()
            .write(Vec3::expr(self.index.x, self.index.y, index), value);
    }
    fn can_write(&self) -> bool {
        true
    }
}

impl<V: Value, T: EmanationType> Reference<'_, Field<Slice<Expr<V>>, T>> {
    #[tracked]
    pub fn bind_array_slices(
        self,
        index: impl LinearIndex<T>,
        slice_size: u32,
        check_bounds: bool,
        values: impl IntoBuffer<V>,
    ) -> Self {
        let index = index.reduce();
        let (buffer, handle) = values.into_buffer(self.device(), index.size() * slice_size);
        self.bind_fn(move |el| {
            let _handle = &handle;
            let buffer = buffer.clone();
            let offset = index[[el]] * slice_size;
            Slice {
                size: slice_size,
                check_bounds,
                access: Arc::new(BufferSliceAccessor { buffer, offset }),
            }
        })
    }
    #[tracked]
    pub fn bind_tex2d_slices(
        self,
        index: impl LinearIndex<T>,
        slice_size: u32,
        check_bounds: bool,
        storage: PixelStorage,
    ) -> Self
    where
        V: IoTexel,
    {
        let index = index.reduce();
        let texture = self
            .device()
            .create_tex2d(storage, index.size(), slice_size, 1);
        self.bind_fn(move |el| {
            let texture = texture.view(0);
            let index = index[[el]];
            Slice {
                size: slice_size,
                check_bounds,
                access: Arc::new(Tex2dSliceAccessor { texture, index }),
            }
        })
    }
    #[tracked]
    pub fn bind_tex3d_slices(
        self,
        index: impl PlanarIndex<T>,
        slice_size: u32,
        check_bounds: bool,
        storage: PixelStorage,
    ) -> Self
    where
        V: IoTexel,
    {
        let index = index.reduce();
        let texture =
            self.device()
                .create_tex3d(storage, index.size().x, index.size().y, slice_size, 1);
        self.bind_fn(move |el| {
            let texture = texture.view(0);
            let index = index[[el]];
            Slice {
                size: slice_size,
                check_bounds,
                access: Arc::new(Tex3dSliceAccessor { texture, index }),
            }
        })
    }
}
