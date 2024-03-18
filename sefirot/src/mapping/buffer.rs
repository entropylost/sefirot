use std::ops::Deref;
use std::sync::Arc;

use luisa::lang::types::vector::{Vec2, Vec3};
use luisa::lang::types::AtomicRef;

use super::cache::SimpleExprMapping;
use crate::domain::{DispatchArgs, Domain};
use crate::graph::NodeConfigs;
use crate::internal_prelude::*;
use crate::kernel::KernelContext;
use crate::mapping::cache::impl_cache_mapping;
use crate::tracked_nc;

mod storage;
pub use storage::HasPixelStorage;

// TODO: Offer ways of creating buffers of the correct size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StaticDomain<const N: usize>(pub [u32; N]);
impl StaticDomain<1> {
    pub fn new(size: u32) -> Self {
        Self([size])
    }
    pub fn map_buffer<V: Value>(
        &self,
        buffer: impl IntoHandled<H = HandledBuffer<V>>,
    ) -> BufferMapping<V> {
        let buffer = buffer.into_handled();
        debug_assert_eq!(buffer.len() as u32, self.len());
        BufferMapping(buffer)
    }
    pub fn create_buffer<V: Value>(&self, device: &Device) -> BufferMapping<V> {
        let buffer = device.create_buffer::<V>(self.len() as usize);
        self.map_buffer(buffer)
    }
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> u32 {
        self.0[0]
    }
    pub fn width(&self) -> u32 {
        self.0[0]
    }
}
impl StaticDomain<2> {
    pub fn new(width: u32, height: u32) -> Self {
        Self([width, height])
    }
    pub fn map_tex2d<V: IoTexel>(
        &self,
        texture: impl IntoHandled<H = HandledTex2d<V>>,
    ) -> Tex2dMapping<V> {
        let texture = texture.into_handled();
        debug_assert_eq!(texture.size()[0..2], self.0);
        Tex2dMapping(texture)
    }
    pub fn create_tex2d<V: HasPixelStorage>(&self, device: &Device) -> Tex2dMapping<V> {
        self.create_tex2d_with_storage(device, V::storage())
    }
    pub fn create_tex2d_with_storage<V: IoTexel>(
        &self,
        device: &Device,
        storage: PixelStorage,
    ) -> Tex2dMapping<V> {
        let texture = device.create_tex2d::<V>(storage, self.width(), self.height(), 1);
        self.map_tex2d(texture)
    }
    pub fn width(&self) -> u32 {
        self.0[0]
    }
    pub fn height(&self) -> u32 {
        self.0[1]
    }
}
impl StaticDomain<3> {
    pub fn new(width: u32, height: u32, depth: u32) -> Self {
        Self([width, height, depth])
    }
    pub fn map_tex3d<V: IoTexel>(
        &self,
        texture: impl IntoHandled<H = HandledTex3d<V>>,
    ) -> Tex3dMapping<V> {
        let texture = texture.into_handled();
        debug_assert_eq!(texture.size(), self.0);
        Tex3dMapping(texture)
    }
    pub fn create_tex3d<V: HasPixelStorage>(&self, device: &Device) -> Tex3dMapping<V> {
        self.create_tex3d_with_storage(device, V::storage())
    }
    pub fn create_tex3d_with_storage<V: IoTexel>(
        &self,
        device: &Device,
        storage: PixelStorage,
    ) -> Tex3dMapping<V> {
        let texture =
            device.create_tex3d::<V>(storage, self.width(), self.height(), self.depth(), 1);
        self.map_tex3d(texture)
    }
    pub fn width(&self) -> u32 {
        self.0[0]
    }
    pub fn height(&self) -> u32 {
        self.0[1]
    }
    pub fn depth(&self) -> u32 {
        self.0[2]
    }
}

impl Domain for StaticDomain<1> {
    type A = ();
    type I = Expr<u32>;
    fn get_element(&self, kernel_context: Arc<KernelContext>) -> Element<Self::I> {
        Element::new(dispatch_id().x, Context::new(kernel_context))
    }
    fn dispatch_async(&self, _domain_args: (), args: DispatchArgs) -> NodeConfigs<'static> {
        args.dispatch([self.0[0], 1, 1])
    }
    #[tracked_nc]
    fn contains(&self, index: &Self::I) -> Expr<bool> {
        *index < self.0[0]
    }
}
impl Domain for StaticDomain<2> {
    type A = ();
    type I = Expr<Vec2<u32>>;
    fn get_element(&self, kernel_context: Arc<KernelContext>) -> Element<Self::I> {
        Element::new(dispatch_id().xy(), Context::new(kernel_context))
    }
    fn dispatch_async(&self, _domain_args: (), args: DispatchArgs) -> NodeConfigs<'static> {
        args.dispatch([self.0[0], self.0[1], 1])
    }
    #[tracked_nc]
    fn contains(&self, index: &Self::I) -> Expr<bool> {
        (index < Vec2::from(self.0)).all()
    }
}
impl Domain for StaticDomain<3> {
    type A = ();
    type I = Expr<Vec3<u32>>;
    fn get_element(&self, kernel_context: Arc<KernelContext>) -> Element<Self::I> {
        Element::new(dispatch_id(), Context::new(kernel_context))
    }
    fn dispatch_async(&self, _domain_args: (), args: DispatchArgs) -> NodeConfigs<'static> {
        args.dispatch(self.0)
    }
    #[tracked_nc]
    fn contains(&self, index: &Self::I) -> Expr<bool> {
        (index < Vec3::from(self.0)).all()
    }
}

pub struct HandledBuffer<V: Value> {
    pub buffer: BufferView<V>,
    pub handle: Option<Buffer<V>>,
}
impl<V: Value> Deref for HandledBuffer<V> {
    type Target = BufferView<V>;
    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}
pub struct HandledTex2d<V: IoTexel> {
    pub texture: Tex2dView<V>,
    pub handle: Option<Tex2d<V>>,
}
impl<V: IoTexel> Deref for HandledTex2d<V> {
    type Target = Tex2dView<V>;
    fn deref(&self) -> &Self::Target {
        &self.texture
    }
}
pub struct HandledTex3d<V: IoTexel> {
    pub texture: Tex3dView<V>,
    pub handle: Option<Tex3d<V>>,
}
impl<V: IoTexel> Deref for HandledTex3d<V> {
    type Target = Tex3dView<V>;
    fn deref(&self) -> &Self::Target {
        &self.texture
    }
}

pub trait IntoHandled {
    type H;
    fn into_handled(self) -> Self::H;
}
impl<V: Value> IntoHandled for BufferView<V> {
    type H = HandledBuffer<V>;
    fn into_handled(self) -> Self::H {
        HandledBuffer {
            buffer: self,
            handle: None,
        }
    }
}
impl<V: Value> IntoHandled for Buffer<V> {
    type H = HandledBuffer<V>;
    fn into_handled(self) -> Self::H {
        HandledBuffer {
            buffer: self.view(..),
            handle: Some(self),
        }
    }
}
impl<V: IoTexel> IntoHandled for Tex2dView<V> {
    type H = HandledTex2d<V>;
    fn into_handled(self) -> Self::H {
        HandledTex2d {
            texture: self,
            handle: None,
        }
    }
}
impl<V: IoTexel> IntoHandled for Tex2d<V> {
    type H = HandledTex2d<V>;
    fn into_handled(self) -> Self::H {
        HandledTex2d {
            texture: self.view(0),
            handle: Some(self),
        }
    }
}
impl<V: IoTexel> IntoHandled for Tex3dView<V> {
    type H = HandledTex3d<V>;
    fn into_handled(self) -> Self::H {
        HandledTex3d {
            texture: self,
            handle: None,
        }
    }
}
impl<V: IoTexel> IntoHandled for Tex3d<V> {
    type H = HandledTex3d<V>;
    fn into_handled(self) -> Self::H {
        HandledTex3d {
            texture: self.view(0),
            handle: Some(self),
        }
    }
}

pub struct BufferMapping<V: Value>(pub HandledBuffer<V>);
impl<V: Value> SimpleExprMapping<V, Expr<u32>> for BufferMapping<V> {
    fn get_expr(&self, index: &Expr<u32>, _ctx: &mut Context, _binding: FieldId) -> Expr<V> {
        self.0.read(*index)
    }
    fn set_expr(&self, index: &Expr<u32>, value: Expr<V>, _ctx: &mut Context, _binding: FieldId) {
        self.0.write(*index, value);
    }
}
impl_cache_mapping!([V: Value] Mapping[V, Expr<u32>] for BufferMapping<V>);
impl<V: Value> Mapping<AtomicRef<V>, Expr<u32>> for BufferMapping<V> {
    fn access(&self, index: &Expr<u32>, _ctx: &mut Context, _binding: FieldId) -> AtomicRef<V> {
        self.0.atomic_ref(*index)
    }
}

pub struct Tex2dMapping<V: IoTexel>(pub HandledTex2d<V>);
impl<V: IoTexel> SimpleExprMapping<V, Expr<Vec2<u32>>> for Tex2dMapping<V> {
    fn get_expr(&self, index: &Expr<Vec2<u32>>, _ctx: &mut Context, _binding: FieldId) -> Expr<V> {
        self.0.read(*index)
    }
    fn set_expr(
        &self,
        index: &Expr<Vec2<u32>>,
        value: Expr<V>,
        _ctx: &mut Context,
        _binding: FieldId,
    ) {
        self.0.write(*index, value);
    }
}
impl_cache_mapping!([V: IoTexel] Mapping[V, Expr<Vec2<u32>>] for Tex2dMapping<V>);

pub struct Tex3dMapping<V: IoTexel>(pub HandledTex3d<V>);
impl<V: IoTexel> SimpleExprMapping<V, Expr<Vec3<u32>>> for Tex3dMapping<V> {
    fn get_expr(&self, index: &Expr<Vec3<u32>>, _ctx: &mut Context, _binding: FieldId) -> Expr<V> {
        self.0.read(*index)
    }
    fn set_expr(
        &self,
        index: &Expr<Vec3<u32>>,
        value: Expr<V>,
        _ctx: &mut Context,
        _binding: FieldId,
    ) {
        self.0.write(*index, value);
    }
}
impl_cache_mapping!([V: IoTexel] Mapping[V, Expr<Vec3<u32>>] for Tex3dMapping<V>);
