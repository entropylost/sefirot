use std::cell::Cell;
use std::sync::Arc;

use encoder::LinearEncoder;
use patterns::{CheckerboardPattern, MargolusPattern};
use sefirot::ext_prelude::*;
use sefirot::field::FieldHandle;
use sefirot::luisa::lang::types::vector::Vec2;
use sefirot::luisa::lang::types::AtomicRef;
use sefirot::mapping::buffer::{
    HandledBuffer, HandledTex2d, HasPixelStorage, IntoHandled, StaticDomain,
};
use sefirot::mapping::function::{CachedFnMapping, FnMapping};
use sefirot::mapping::index::IndexMap;
use sefirot::mapping::AMapping;

pub mod dual;
pub mod encoder;
pub mod patterns;

#[derive(Debug)]
pub struct GridDomain {
    index: EField<Vec2<u32>, Vec2<i32>>,
    _index_handle: Option<FieldHandle>,
    encoder: Option<LinearEncoder>,
    start: [i32; 2],
    shifted_domain: StaticDomain<2>,
    wrapping: bool,
}
impl Clone for GridDomain {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            _index_handle: None,
            encoder: self.encoder.clone(),
            start: self.start,
            shifted_domain: self.shifted_domain,
            wrapping: self.wrapping,
        }
    }
}
impl DomainImpl for GridDomain {
    type Args = ();
    type Index = Expr<Vec2<i32>>;
    type Passthrough = ();
    #[tracked_nc]
    fn get_element(&self, kernel_context: Arc<KernelContext>, _: ()) -> Element<Self::Index> {
        let index = dispatch_id().xy().cast_i32() + Vec2::from(self.start);
        let mut context = Context::new(kernel_context);
        context.bind_local(self.index, FnMapping::new(|_el, _ctx| dispatch_id().xy()));
        Element::new(index, context)
    }
    fn dispatch(&self, _: Self::Args, args: KernelDispatch) -> NodeConfigs<'static> {
        args.dispatch([self.size()[0], self.size()[1], 1])
    }
    #[tracked_nc]
    fn contains_impl(&self, index: &Self::Index) -> Expr<bool> {
        if self.wrapping {
            true.expr()
        } else {
            (index >= Vec2::from(self.start)).all() && (index < Vec2::from(self.end())).all()
        }
    }
}

impl GridDomain {
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
        Self {
            index,
            _index_handle: Some(handle),
            encoder: None,
            start,
            shifted_domain: StaticDomain(size),
            wrapping,
        }
    }
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
    pub fn width(&self) -> u32 {
        self.size()[0]
    }
    pub fn height(&self) -> u32 {
        self.size()[1]
    }

    pub fn encoder(&self) -> &LinearEncoder {
        self.encoder.as_ref().unwrap()
    }
    pub fn get_encoder(&self) -> Option<&LinearEncoder> {
        self.encoder.as_ref()
    }

    pub fn with_encoder(self, encoder: LinearEncoder) -> Self {
        debug_assert!(encoder.allowed_size(self.size()));
        Self {
            encoder: Some(encoder),
            ..self
        }
    }
    pub fn with_morton(self) -> Self {
        self.with_encoder(LinearEncoder::morton())
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
    pub fn map_buffer<V: Value>(
        &self,
        buffer: impl IntoHandled<H = HandledBuffer<V>>,
    ) -> impl AMapping<V, Vec2<i32>> {
        IndexMap::new(
            self.index,
            self.encoder
                .as_ref()
                .expect("Mapping a buffer needs a LinearEncoder")
                .encode::<AtomicRef<V>, _>(
                    StaticDomain::<1>::new(self.size()[0] * self.size()[1]).map_buffer(buffer),
                ),
        )
    }
    pub fn create_buffer<V: Value>(&self, device: &Device) -> impl AMapping<V, Vec2<i32>> {
        self.map_buffer(device.create_buffer((self.size()[0] * self.size()[1]) as usize))
    }

    pub fn checkerboard(&self) -> CheckerboardPattern {
        debug_assert_eq!(self.width() % 2, 0, "Checkerboard pattern needs even size");
        debug_assert_eq!(self.height() % 2, 0, "Checkerboard pattern needs even size");

        CheckerboardPattern { grid: self.clone() }
    }
    pub fn margolus(&self) -> MargolusPattern {
        if self.wrapping {
            debug_assert_eq!(
                self.width() % 2,
                0,
                "Margolus pattern on wrapping world needs even size"
            );
            debug_assert_eq!(
                self.height() % 2,
                0,
                "Margolus pattern on wrapping world needs even size"
            );
        }

        MargolusPattern { grid: self.clone() }
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
