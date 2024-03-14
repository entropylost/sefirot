use std::sync::Arc;

use luisa::lang::types::vector::{Vec2, Vec3};
use luisa::lang::types::AtomicRef;

use super::cache::{SimpleExprMapping, VarCacheMapping};
use crate::domain::{AsEntireDomain, DispatchArgs, Domain};
use crate::graph::{AsNodes, NodeConfigs};
use crate::internal_prelude::*;
use crate::kernel::KernelContext;

pub struct StaticDomain<const N: usize>(pub [u32; N]);
impl Domain for StaticDomain<1> {
    type A = ();
    type I = Expr<u32>;
    fn get_element(&self, kernel_context: Arc<KernelContext>) -> Element<Self::I> {
        Element {
            index: dispatch_id().x,
            context: Context::new(kernel_context),
        }
    }
    fn dispatch_async(&self, _domain_args: (), args: DispatchArgs) -> NodeConfigs<'static> {
        args.dispatch([self.0[0], 1, 1]).into_node_configs()
    }
}
impl Domain for StaticDomain<2> {
    type A = ();
    type I = Expr<Vec2<u32>>;
    fn get_element(&self, kernel_context: Arc<KernelContext>) -> Element<Self::I> {
        Element {
            index: dispatch_id().xy(),
            context: Context::new(kernel_context),
        }
    }
    fn dispatch_async(&self, _domain_args: (), args: DispatchArgs) -> NodeConfigs<'static> {
        args.dispatch([self.0[0], self.0[1], 1]).into_node_configs()
    }
}
impl Domain for StaticDomain<3> {
    type A = ();
    type I = Expr<Vec3<u32>>;
    fn get_element(&self, kernel_context: Arc<KernelContext>) -> Element<Self::I> {
        Element {
            index: dispatch_id(),
            context: Context::new(kernel_context),
        }
    }
    fn dispatch_async(&self, _domain_args: (), args: DispatchArgs) -> NodeConfigs<'static> {
        args.dispatch(self.0).into_node_configs()
    }
}

pub struct BufferMapping<V: Value> {
    buffer: BufferView<V>,
    handle: Option<Buffer<V>>,
}
impl<V: Value> BufferMapping<V> {
    pub fn from_buffer(buffer: Buffer<V>) -> Self {
        let view = buffer.view(..);
        Self {
            buffer: view,
            handle: Some(buffer),
        }
    }
    pub fn from_view(view: BufferView<V>) -> Self {
        Self {
            buffer: view,
            handle: None,
        }
    }
    pub fn from_slice(device: &Device, data: &[V]) -> Self {
        let buffer = device.create_buffer_from_slice(data);
        Self::from_buffer(buffer)
    }
    pub fn from_size(device: &Device, size: usize) -> Self {
        let buffer = device.create_buffer(size);
        Self::from_buffer(buffer)
    }
    pub fn view(&self) -> &BufferView<V> {
        &self.buffer
    }
    pub fn buffer(&self) -> &Option<Buffer<V>> {
        &self.handle
    }
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }
}
impl<V: Value> AsEntireDomain for BufferMapping<V> {
    type Entire = StaticDomain<1>;
    fn entire_domain(&self) -> Self::Entire {
        StaticDomain([self.len() as u32])
    }
}
impl<V: Value> SimpleExprMapping<V, Expr<u32>> for BufferMapping<V> {
    fn get_expr(&self, index: &Expr<u32>, _ctx: &mut Context, _binding: FieldHandle) -> Expr<V> {
        self.buffer.read(*index)
    }
    fn set_expr(
        &self,
        index: &Expr<u32>,
        value: Expr<V>,
        _ctx: &mut Context,
        _binding: FieldHandle,
    ) {
        self.buffer.write(*index, value);
    }
}
// TODO: Figure out how to make this work without needing same-crate impl.
// Perhaps offer a separate InternalCacheMapping?
impl<V: Value> Mapping<AtomicRef<V>, Expr<u32>> for VarCacheMapping<BufferMapping<V>> {
    fn access(&self, index: &Expr<u32>, _ctx: &mut Context, _binding: FieldHandle) -> AtomicRef<V> {
        self.0.buffer.atomic_ref(*index)
    }
}

pub struct Tex2dMapping<V: IoTexel> {
    texture: Tex2dView<V>,
    _handle: Option<Tex2d<V>>,
}
// TODO: Non-zero layers are not supported.
impl<V: IoTexel> Tex2dMapping<V> {
    pub fn from_texture(texture: Tex2d<V>) -> Self {
        let view = texture.view(0);
        Self {
            texture: view,
            _handle: Some(texture),
        }
    }
    // View must be on level 0.
    pub fn from_view(view: Tex2dView<V>) -> Self {
        Self {
            texture: view,
            _handle: None,
        }
    }
    pub fn size(&self) -> [u32; 2] {
        let size = self.texture.size();
        [size[0], size[1]]
    }
}
impl<V: IoTexel> AsEntireDomain for Tex2dMapping<V> {
    type Entire = StaticDomain<2>;
    fn entire_domain(&self) -> Self::Entire {
        StaticDomain(self.size())
    }
}
impl<V: IoTexel> SimpleExprMapping<V, Expr<Vec2<u32>>> for Tex2dMapping<V> {
    fn get_expr(
        &self,
        index: &Expr<Vec2<u32>>,
        _ctx: &mut Context,
        _binding: FieldHandle,
    ) -> Expr<V> {
        self.texture.read(*index)
    }
    fn set_expr(
        &self,
        index: &Expr<Vec2<u32>>,
        value: Expr<V>,
        _ctx: &mut Context,
        _binding: FieldHandle,
    ) {
        self.texture.write(*index, value);
    }
}

pub struct Tex3dMapping<V: IoTexel> {
    texture: Tex3dView<V>,
    _handle: Option<Tex3d<V>>,
}
// TODO: Non-zero layers are not supported.
impl<V: IoTexel> Tex3dMapping<V> {
    pub fn from_texture(texture: Tex3d<V>) -> Self {
        let view = texture.view(0);
        Self {
            texture: view,
            _handle: Some(texture),
        }
    }
    // View must be on level 0.
    pub fn from_view(view: Tex3dView<V>) -> Self {
        Self {
            texture: view,
            _handle: None,
        }
    }
    pub fn size(&self) -> [u32; 3] {
        let size = self.texture.size();
        [size[0], size[1], size[2]]
    }
}
impl<V: IoTexel> AsEntireDomain for Tex3dMapping<V> {
    type Entire = StaticDomain<3>;
    fn entire_domain(&self) -> Self::Entire {
        StaticDomain(self.size())
    }
}
impl<V: IoTexel> SimpleExprMapping<V, Expr<Vec3<u32>>> for Tex3dMapping<V> {
    fn get_expr(
        &self,
        index: &Expr<Vec3<u32>>,
        _ctx: &mut Context,
        _binding: FieldHandle,
    ) -> Expr<V> {
        self.texture.read(*index)
    }
    fn set_expr(
        &self,
        index: &Expr<Vec3<u32>>,
        value: Expr<V>,
        _ctx: &mut Context,
        _binding: FieldHandle,
    ) {
        self.texture.write(*index, value);
    }
}
