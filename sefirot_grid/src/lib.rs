use std::sync::Arc;

use sefirot::ext_prelude::*;
use sefirot::field::FieldHandle;
use sefirot::luisa::lang::types::vector::Vec2;
use sefirot::mapping::buffer::{HandledTex2d, IntoHandled, StaticDomain};
use sefirot::mapping::function::CachedFnMapping;
use sefirot::mapping::index::IndexMap;

// TODO: Actually make this useful.

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GridDomain {
    index: EField<Vec2<u32>, Vec2<i32>>,
    index_handle: Arc<FieldHandle>,
    start: [i32; 2],
    shifted_domain: StaticDomain<2>,
}
impl Domain for GridDomain {
    type A = ();
    type I = Expr<Vec2<i32>>;
    #[tracked(crate = "sefirot::luisa")]
    fn get_element(&self, kernel_context: Arc<KernelContext>) -> Element<Self::I> {
        Element {
            index: dispatch_id().xy().cast_i32() + Vec2::from(self.start),
            context: Context::new(kernel_context),
        }
    }
    fn dispatch_async(&self, _domain_args: Self::A, args: DispatchArgs) -> NodeConfigs<'static> {
        args.dispatch([self.size()[0], self.size()[1], 1])
            .into_node_configs()
    }
}
impl IndexDomain for GridDomain {
    fn get_index(&self, index: &Self::I, kernel_context: Arc<KernelContext>) -> Element<Self::I> {
        Element {
            index: *index,
            context: Context::new(kernel_context),
        }
    }
    #[tracked(crate = "sefirot::luisa")]
    fn get_index_fallable(
        &self,
        index: &Self::I,
        kernel_context: Arc<KernelContext>,
    ) -> (Element<Self::I>, Expr<bool>) {
        (
            self.get_index(index, kernel_context),
            (index >= Vec2::from(self.start)).all() && (index < Vec2::from(self.end())).all(),
        )
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
        let (index, handle) = Field::create_bind(
            "grid-index",
            CachedFnMapping::<Expr<Vec2<u32>>, Expr<Vec2<i32>>, _>::new(
                move |index, _ctx| track!(crate = "sefirot::luisa" => (index - Vec2::from(start)).cast_u32()),
            ),
        );
        Self {
            index,
            index_handle: Arc::new(handle),
            start,
            shifted_domain: StaticDomain(size),
        }
    }
    pub fn map_texture<V: IoTexel>(
        &self,
        texture: impl IntoHandled<H = HandledTex2d<V>>,
    ) -> impl VMapping<V, Vec2<i32>> {
        IndexMap::new(self.index, self.shifted_domain.map_tex2d(texture))
    }
}
