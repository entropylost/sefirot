use std::cell::Cell;
use std::sync::Arc;

use sefirot::ext_prelude::*;
use sefirot::field::FieldHandle;
use sefirot::luisa::lang::types::vector::Vec2;
use sefirot::mapping::buffer::{
    HandledBuffer, HandledTex2d, HasPixelStorage, IntoHandled, StaticDomain,
};
use sefirot::mapping::function::{CachedFnMapping, FnMapping};
use sefirot::mapping::index::IndexMap;
use sefirot::mapping::AMapping;

// TODO: Actually make this useful.

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GridDomain {
    index: EField<Vec2<u32>, Vec2<i32>>,
    index_handle: Arc<FieldHandle>,
    morton_index: Option<EField<u32, Vec2<i32>>>,
    morton_handle: Option<Arc<FieldHandle>>,
    start: [i32; 2],
    shifted_domain: StaticDomain<2>,
    wrapping: bool,
}
impl Domain for GridDomain {
    type A = ();
    type I = Expr<Vec2<i32>>;
    #[tracked_nc]
    fn get_element(&self, kernel_context: Arc<KernelContext>) -> Element<Self::I> {
        let index = dispatch_id().xy().cast_i32() + Vec2::from(self.start);
        let mut context = Context::new(kernel_context);
        context.bind_local(self.index, FnMapping::new(|_el, _ctx| dispatch_id().xy()));
        Element::new(index, context)
    }
    fn dispatch_async(&self, _domain_args: Self::A, args: DispatchArgs) -> NodeConfigs<'static> {
        args.dispatch([self.size()[0], self.size()[1], 1])
    }
    #[tracked_nc]
    fn contains(&self, index: &Self::I) -> Expr<bool> {
        if self.wrapping {
            true.expr()
        } else {
            (index >= Vec2::from(self.start)).all() && (index < Vec2::from(self.end())).all()
        }
    }
}

impl GridDomain {
    pub fn start(&self) -> [i32; 2] {
        self.start
    }
    pub fn size(&self) -> [u32; 2] {
        self.shifted_domain.0
    }
    pub fn end(&self) -> [i32; 2] {
        [
            self.start[0] + self.shifted_domain.0[0] as i32,
            self.start[1] + self.shifted_domain.0[1] as i32,
        ]
    }
    pub fn new(start: [i32; 2], size: [u32; 2]) -> Self {
        Self::new_with_wrapping(start, size, false)
    }
    pub fn new_wrapping(start: [i32; 2], size: [u32; 2]) -> Self {
        Self::new_with_wrapping(start, size, true)
    }
    pub fn new_with_wrapping(start: [i32; 2], size: [u32; 2], wrapping: bool) -> Self {
        let (index, handle) = Field::create_bind(
            "grid-index",
            CachedFnMapping::<Expr<Vec2<u32>>, Expr<Vec2<i32>>, _>::new(track_nc!(
                move |index, _ctx| {
                    if wrapping {
                        let size = Vec2::new(size[0] as i32, size[1] as i32);
                        (((index - Vec2::from(start)) % size + size) % size).cast_u32()
                    } else {
                        (index - Vec2::from(start)).cast_u32()
                    }
                }
            )),
        );
        let (morton_index, morton_handle) =
            if size[0] == size[1] && size[0].is_power_of_two() && size[0] <= 1 << 16 {
                let (morton_index, morton_handle) = Field::create_bind(
                    "grid-morton-index",
                    IndexMap::new(
                        index,
                        CachedFnMapping::<Expr<u32>, Expr<Vec2<u32>>, _>::new(track_nc!(
                            move |index, _ctx| {
                                // https://graphics.stanford.edu/%7Eseander/bithacks.html#InterleaveBMN
                                // TODO: Apparently it's possible to implement this with half as much computations
                                // but it requires u64s: see https://docs.rs/morton/0.3.0/src/morton/lib.rs.html

                                let x = index.x.var();

                                *x = (x | (x << 8)) & 0x00ff00ff;
                                *x = (x | (x << 4)) & 0x0f0f0f0f; // 0b00001111
                                *x = (x | (x << 2)) & 0x33333333; // 0b00110011
                                *x = (x | (x << 1)) & 0x55555555; // 0b01010101

                                let y = index.y.var();

                                *y = (y | (y << 8)) & 0x00ff00ff;
                                *y = (y | (y << 4)) & 0x0f0f0f0f; // 0b00001111
                                *y = (y | (y << 2)) & 0x33333333; // 0b00110011
                                *y = (y | (y << 1)) & 0x55555555; // 0b01010101

                                x | (y << 1)
                            }
                        )),
                    ),
                );
                (Some(morton_index), Some(Arc::new(morton_handle)))
            } else {
                (None, None)
            };
        Self {
            index,
            index_handle: Arc::new(handle),
            morton_index,
            morton_handle,
            start,
            shifted_domain: StaticDomain(size),
            wrapping,
        }
    }
    pub fn map_texture<V: IoTexel>(
        &self,
        texture: impl IntoHandled<H = HandledTex2d<V>>,
    ) -> impl VMapping<V, Vec2<i32>> {
        IndexMap::new(self.index, self.shifted_domain.map_tex2d(texture))
    }
    pub fn create_texture<V: HasPixelStorage>(
        &self,
        device: &Device,
    ) -> impl VMapping<V, Vec2<i32>> {
        self.create_texture_with_storage(device, V::storage())
    }
    pub fn create_texture_with_storage<V: IoTexel>(
        &self,
        device: &Device,
        storage: PixelStorage,
    ) -> impl VMapping<V, Vec2<i32>> {
        self.map_texture(device.create_tex2d(storage, self.size()[0], self.size()[1], 1))
    }
    pub fn map_buffer_morton<V: Value>(
        &self,
        buffer: impl IntoHandled<H = HandledBuffer<V>>,
    ) -> impl AMapping<V, Vec2<i32>> {
        let morton_index = self
            .morton_index
            .expect("Morton index is not available, due to a non power-of-two square size");
        IndexMap::new(
            morton_index,
            StaticDomain::<1>::new(self.size()[0] * self.size()[1]).map_buffer(buffer),
        )
    }
    pub fn create_buffer_morton<V: Value>(&self, device: &Device) -> impl AMapping<V, Vec2<i32>> {
        self.map_buffer_morton(device.create_buffer((self.size()[0] * self.size()[1]) as usize))
    }
    #[tracked]
    pub fn on_adjacent(&self, el: &Element<Expr<Vec2<i32>>>, f: impl Fn(Element<Expr<Vec2<i32>>>)) {
        for dir in [Vec2::x(), Vec2::y(), -Vec2::x(), -Vec2::y()] {
            let el = el.at(**el + dir);
            let within = self.contains(&el);
            let cell = Cell::new(Some(el));
            if within {
                f(cell.take().unwrap());
            }
        }
    }
}
