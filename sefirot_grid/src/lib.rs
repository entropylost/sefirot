use std::cell::Cell as StdCell;
use std::rc::Rc;
use std::sync::Arc;

use dual::DualGrid;
use encoder::LinearEncoder;
use offset::OffsetDomain;
use parking_lot::RwLock;
use patterns::{CheckerboardPattern, MargolusPattern};
use sefirot::ext_prelude::*;
use sefirot::field::FieldHandle;
use sefirot::luisa::lang::types::vector::Vec2;
use sefirot::luisa::lang::types::AtomicRef;
use sefirot::mapping::bindless::{
    BindlessBufferHandle, BindlessBufferMapping, BindlessMapper, Emplace,
};
use sefirot::mapping::buffer::{
    BufferMapping, HandledBuffer, HandledTex2d, HasPixelStorage, IntoHandled, StaticDomain,
};
use sefirot::mapping::function::{CachedFnMapping, FnMapping};
use sefirot::mapping::index::IndexMap;

pub mod dual;
pub mod encoder;
pub mod offset;
pub mod patterns;
pub mod tiled;

pub type Cell = Expr<Vec2<i32>>;

#[derive(Debug, Value, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GridDirection {
    Left = 0,
    Down = 1,
    Right = 2,
    Up = 3,
}
impl GridDirection {
    pub fn iter_all() -> [GridDirection; 4] {
        [
            GridDirection::Left,
            GridDirection::Down,
            GridDirection::Right,
            GridDirection::Up,
        ]
    }
    pub fn sign(&self) -> i32 {
        match self {
            GridDirection::Left => -1,
            GridDirection::Down => -1,
            GridDirection::Right => 1,
            GridDirection::Up => 1,
        }
    }
    pub fn signf(&self) -> f32 {
        self.sign() as f32
    }
    pub fn as_vec(&self) -> Vec2<i32> {
        match self {
            GridDirection::Left => Vec2::new(-1, 0),
            GridDirection::Down => Vec2::new(0, -1),
            GridDirection::Right => Vec2::new(1, 0),
            GridDirection::Up => Vec2::new(0, 1),
        }
    }
    // TODO: Generalize.
    #[tracked_nc]
    pub fn extract(&self, value: Expr<Vec2<f32>>) -> Expr<f32> {
        match self {
            GridDirection::Left => -value.x,
            GridDirection::Down => -value.y,
            GridDirection::Right => value.x,
            GridDirection::Up => value.y,
        }
    }
    pub fn as_vec_f32(&self) -> Vec2<f32> {
        let v = self.as_vec();
        Vec2::new(v.x as f32, v.y as f32)
    }
    pub fn rotate_ccw(&self) -> Self {
        match self {
            GridDirection::Left => GridDirection::Down,
            GridDirection::Down => GridDirection::Right,
            GridDirection::Right => GridDirection::Up,
            GridDirection::Up => GridDirection::Left,
        }
    }
    pub fn rotate_cw(&self) -> Self {
        match self {
            GridDirection::Left => GridDirection::Up,
            GridDirection::Down => GridDirection::Left,
            GridDirection::Right => GridDirection::Down,
            GridDirection::Up => GridDirection::Right,
        }
    }
}

#[derive(Debug)]
pub struct GridDomain {
    index: EField<Vec2<u32>, Cell>,
    _index_handle: Option<FieldHandle>,
    encoder: Option<LinearEncoder>,
    offset: Arc<RwLock<Vec2<i32>>>,
    offset_field: EField<Vec2<i32>, ()>,
    _offset_handle: Option<FieldHandle>,

    shifted_domain: StaticDomain<2>,
    wrapping: bool,
}
impl Clone for GridDomain {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            _index_handle: None,
            encoder: self.encoder.clone(),
            offset: self.offset.clone(),
            offset_field: self.offset_field,
            _offset_handle: None,
            shifted_domain: self.shifted_domain,
            wrapping: self.wrapping,
        }
    }
}
impl DomainImpl for GridDomain {
    type Args = ();
    type Index = Cell;
    type Passthrough = ();
    #[tracked_nc]
    fn get_element(&self, kernel_context: Rc<KernelContext>, _: ()) -> Element<Self::Index> {
        let mut context = Context::new(kernel_context);
        let index = dispatch_id().xy().cast_i32() + self.offset_field.at_split(&(), &mut context);
        context.bind_local(self.index, FnMapping::new(|_idx, _ctx| dispatch_id().xy()));
        Element::new(index, context)
    }
    fn dispatch(&self, _: Self::Args, args: KernelDispatch) -> NodeConfigs<'static> {
        args.dispatch([self.size()[0], self.size()[1], 1])
    }
    #[tracked_nc]
    fn contains_impl(&self, el: &Element<Self::Index>) -> Expr<bool> {
        if self.wrapping {
            true.expr()
        } else {
            let offset = self.offset_field.at_global(el);
            (**el >= offset).all()
                && (**el < Vec2::from(self.size().map(|x| x as i32)) + offset).all()
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
    pub fn new_with_wrapping(starting_offset: [i32; 2], size: [u32; 2], wrapping: bool) -> Self {
        let offset = Arc::new(RwLock::new(Vec2::from(starting_offset)));
        // TODO: Go back to ConstantMapping when it isn't broken.
        let (offset_field, offset_handle) = Field::create_bind(
            "grid-offset",
            FnMapping::new(move |_, _| Vec2::from(starting_offset).expr()),
        );

        let (index, index_handle) = Field::create_bind(
            "grid-index",
            CachedFnMapping::<Expr<Vec2<u32>>, Cell, _>::new(track_nc!(move |index, ctx| {
                let offset = offset_field.at_split(&(), ctx);
                if wrapping {
                    let size = Vec2::new(size[0] as i32, size[1] as i32);
                    (index - offset).rem_euclid(size).cast_u32()
                } else {
                    (index - offset).cast_u32()
                }
            })),
        );
        Self {
            index,
            _index_handle: Some(index_handle),
            encoder: None,
            offset,
            offset_field,
            _offset_handle: Some(offset_handle),
            shifted_domain: StaticDomain(size),
            wrapping,
        }
    }
    pub fn size(&self) -> [u32; 2] {
        self.shifted_domain.0
    }
    // These can change.
    pub fn start(&self) -> [i32; 2] {
        (*self.offset.read()).into()
    }
    pub fn end(&self) -> [i32; 2] {
        let start = self.start();
        let size = self.size();
        [start[0] + size[0] as i32, start[1] + size[1] as i32]
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
    ) -> impl VEMapping<V, Vec2<i32>> {
        IndexMap::new(self.index, self.shifted_domain.map_tex2d(texture))
    }
    pub fn create_texture<V: HasPixelStorage>(&self, device: &Device) -> impl VMapping<V, Cell> {
        self.create_texture_with_storage(device, V::storage())
    }
    pub fn create_texture_with_storage<V: IoTexel>(
        &self,
        device: &Device,
        storage: PixelStorage,
    ) -> impl VMapping<V, Cell> {
        self.map_texture(device.create_tex2d(storage, self.size()[0], self.size()[1], 1))
    }
    #[allow(clippy::type_complexity)]
    fn _map_buffer_typed<V: Value>(
        &self,
        buffer: impl IntoHandled<H = HandledBuffer<V>>,
    ) -> IndexMap<Expr<Vec2<u32>>, IndexMap<Expr<u32>, BufferMapping<V>, Expr<Vec2<u32>>>, Cell>
    {
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
    pub fn map_buffer<V: Value>(
        &self,
        buffer: impl IntoHandled<H = HandledBuffer<V>>,
    ) -> impl AMapping<V, Cell> {
        self._map_buffer_typed(buffer)
    }
    #[allow(clippy::type_complexity)]
    fn _create_buffer_typed<V: Value>(
        &self,
        device: &Device,
    ) -> IndexMap<Expr<Vec2<u32>>, IndexMap<Expr<u32>, BufferMapping<V>, Expr<Vec2<u32>>>, Cell>
    {
        self._map_buffer_typed(device.create_buffer((self.size()[0] * self.size()[1]) as usize))
    }
    pub fn create_buffer<V: Value>(&self, device: &Device) -> impl AMapping<V, Cell> {
        self._create_buffer_typed(device)
    }

    #[allow(clippy::type_complexity)]
    fn _map_bindless_buffer_typed<V: Value>(
        &self,
        bindless: &mut BindlessMapper,
        buffer: impl Emplace<H = BindlessBufferHandle<V>>,
    ) -> IndexMap<
        Expr<Vec2<u32>>,
        IndexMap<Expr<u32>, BindlessBufferMapping<V>, Expr<Vec2<u32>>>,
        Cell,
    > {
        IndexMap::new(
            self.index,
            self.encoder
                .as_ref()
                .expect("Mapping a buffer needs a LinearEncoder")
                .encode::<Var<V>, _>(
                    StaticDomain::<1>::new(self.size()[0] * self.size()[1])
                        .map_bindless_buffer(bindless, buffer),
                ),
        )
    }
    pub fn map_bindless_buffer<V: Value>(
        &self,
        bindless: &mut BindlessMapper,
        buffer: impl Emplace<H = BindlessBufferHandle<V>>,
    ) -> impl VMapping<V, Cell> {
        self._map_bindless_buffer_typed(bindless, buffer)
    }
    #[allow(clippy::type_complexity)]
    fn _create_bindless_buffer_typed<V: Value>(
        &self,
        bindless: &mut BindlessMapper,
        device: &Device,
    ) -> IndexMap<
        Expr<Vec2<u32>>,
        IndexMap<Expr<u32>, BindlessBufferMapping<V>, Expr<Vec2<u32>>>,
        Cell,
    > {
        self._map_bindless_buffer_typed(
            bindless,
            device.create_buffer((self.size()[0] * self.size()[1]) as usize),
        )
    }
    pub fn create_bindless_buffer<V: Value>(
        &self,
        bindless: &mut BindlessMapper,
        device: &Device,
    ) -> impl VMapping<V, Cell> {
        self._create_bindless_buffer_typed(bindless, device)
    }

    pub fn dual(&self) -> DualGrid {
        DualGrid::new(self.clone())
    }

    pub fn offset<D: DomainImpl<Index = Expr<Vec2<u32>>>>(&self, domain: D) -> OffsetDomain<D> {
        OffsetDomain {
            domain,
            offset: self.offset_field,
            index: Some(self.index),
        }
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
    pub fn in_dir(&self, el: &Element<Cell>, dir: GridDirection) -> Element<Cell> {
        el.at(**el + dir.as_vec())
    }

    #[tracked]
    pub fn on_adjacent(&self, el: &Element<Cell>, f: impl Fn(Element<Cell>)) {
        for dir in GridDirection::iter_all() {
            let el = self.in_dir(el, dir);
            let within = self.contains(&el);
            let cell = StdCell::new(Some(el));
            if within {
                f(cell.take().unwrap());
            }
        }
    }
}
