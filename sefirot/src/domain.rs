use std::sync::Arc;

use dyn_clone::DynClone;

use crate::graph::NodeConfigs;
use crate::internal_prelude::*;
use crate::kernel::KernelContext;
use crate::prelude::AsNodes;

pub trait AsKernelContext {
    fn as_kernel_context(&self) -> Arc<KernelContext>;
}
impl AsKernelContext for Arc<KernelContext> {
    fn as_kernel_context(&self) -> Arc<KernelContext> {
        self.clone()
    }
}
impl<I: FieldIndex> AsKernelContext for Element<I> {
    fn as_kernel_context(&self) -> Arc<KernelContext> {
        self.context().kernel.clone()
    }
}
impl AsKernelContext for Context {
    fn as_kernel_context(&self) -> Arc<KernelContext> {
        self.kernel.clone()
    }
}

pub trait IndexDomain: Domain {
    fn get_index(&self, index: &Self::I, kernel_context: Arc<KernelContext>) -> Element<Self::I>;
    // Returns true if the index is within the domain.
    fn get_index_fallable(
        &self,
        index: &Self::I,
        kernel_context: Arc<KernelContext>,
    ) -> (Element<Self::I>, Expr<bool>);
    fn index(&self, index: Self::I, kernel_context: &impl AsKernelContext) -> Element<Self::I> {
        self.get_index(&index, kernel_context.as_kernel_context())
    }
}

/// A trait representing a space across which computations may be performed by calling kernels.
/// This is intentionally very generic, and does not provide any guarantees on how many dispatch calls are generated.
/// For most purposes, [`IndexDomain`] is a conveinent way to implement this trait if only a single dispatch call is necessary.
pub trait Domain: DynClone + Send + Sync + 'static {
    type A: 'static;
    type I: FieldIndex;
    fn get_element(&self, kernel_context: Arc<KernelContext>) -> Element<Self::I>;
    fn dispatch_async(&self, domain_args: Self::A, args: DispatchArgs) -> NodeConfigs<'static>;
}
dyn_clone::clone_trait_object!(<A: 'static, I: FieldIndex> Domain<A = A, I = I>);

impl<A: 'static, I: FieldIndex> Domain for Box<dyn Domain<A = A, I = I>> {
    type A = A;
    type I = I;
    fn get_element(&self, kernel_context: Arc<KernelContext>) -> Element<I> {
        self.as_ref().get_element(kernel_context)
    }
    fn dispatch_async(&self, domain_args: A, args: DispatchArgs) -> NodeConfigs<'static> {
        self.as_ref().dispatch_async(domain_args, args)
    }
}

// TODO: Change; indirect dispatch may be necessary.
pub struct DispatchArgs<'a> {
    pub(crate) call_kernel_async: &'a dyn Fn([u32; 3]) -> Command<'static, 'static>,
    // TODO: Why is this here?
    pub(crate) debug_name: Option<String>,
}
impl DispatchArgs<'_> {
    pub fn dispatch(&self, dispatch_size: [u32; 3]) -> NodeConfigs<'static> {
        let config = (self.call_kernel_async)(dispatch_size).into_node_configs();
        if let Some(name) = self.debug_name.as_deref() {
            config.debug(name)
        } else {
            config
        }
    }
    pub fn dispatch_command(&self, dispatch_size: [u32; 3]) -> Command<'static, 'static> {
        (self.call_kernel_async)(dispatch_size)
    }
    pub fn debug_name(&self) -> Option<&str> {
        self.debug_name.as_deref()
    }
}
