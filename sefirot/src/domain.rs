use std::sync::Arc;

use dyn_clone::DynClone;

use crate::graph::NodeConfigs;
use crate::internal_prelude::*;
use crate::kernel::KernelContext;

pub trait IndexDomain: Domain {
    fn get_index(&self, index: &Self::I, kernel_context: Arc<KernelContext>) -> Element<Self::I>;
    fn get_index_fallable(
        &self,
        index: &Self::I,
        kernel_context: Arc<KernelContext>,
    ) -> (Element<Self::I>, Expr<bool>);
}

/// A trait representing a space across which computations may be performed by calling kernels.
/// This is intentionally very generic, and does not provide any guarantees on how many dispatch calls are generated.
/// For most purposes, [`IndexDomain`] is a conveinent way to implement this trait if only a single dispatch call is necessary.
pub trait Domain: DynClone + Send + Sync + 'static {
    type A: 'static;
    type I: FieldIndex;
    fn get_element(&self, kernel_context: Arc<KernelContext>) -> Element<Self::I>;
    fn dispatch_async(&self, domain_args: Self::A, args: DispatchArgs) -> NodeConfigs<'static>;
    fn into_boxed(self) -> Box<dyn Domain<A = Self::A, I = Self::I>>
    where
        Self: Sized,
    {
        Box::new(self)
    }
}
dyn_clone::clone_trait_object!(<A: 'static, I: FieldIndex> Domain<A = A, I = I>);

pub trait AsEntireDomain {
    type Entire: Domain;
    fn entire_domain(&self) -> Self::Entire;
}

impl<A: 'static, I: FieldIndex> Domain for Box<dyn Domain<A = A, I = I>> {
    type A = A;
    type I = I;
    fn get_element(&self, kernel_context: Arc<KernelContext>) -> Element<I> {
        self.as_ref().get_element(kernel_context)
    }
    fn dispatch_async(&self, domain_args: A, args: DispatchArgs) -> NodeConfigs<'static> {
        self.as_ref().dispatch_async(domain_args, args)
    }
    fn into_boxed(self) -> Box<dyn Domain<A = A, I = I>>
    where
        Self: Sized,
    {
        self
    }
}

// TODO: Change; indirect dispatch may be necessary.
pub struct DispatchArgs<'a> {
    pub(crate) call_kernel_async: &'a dyn Fn([u32; 3]) -> Command<'static, 'static>,
    // TODO: Why is this here?
    pub(crate) debug_name: Option<String>,
}
impl DispatchArgs<'_> {
    pub fn dispatch(&self, dispatch_size: [u32; 3]) -> Command<'static, 'static> {
        (self.call_kernel_async)(dispatch_size)
    }
    pub fn debug_name(&self) -> Option<&str> {
        self.debug_name.as_deref()
    }
}
