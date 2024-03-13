use std::sync::Arc;

use crate::graph::NodeConfigs;
use crate::internal_prelude::*;
use crate::kernel::KernelContext;

/// A trait representing a space across which computations may be performed by calling kernels.
/// This is intentionally very generic, and does not provide any guarantees on how many dispatch calls are generated.
/// For most purposes, [`IndexDomain`] is a conveinent way to implement this trait if only a single dispatch call is necessary.
pub trait Domain: Send + Sync + 'static {
    type A: 'static;
    type T: EmanationType;
    fn get_element(&self, kernel_context: Arc<KernelContext>) -> Element<Self::T>;
    fn dispatch_async(&self, domain_args: Self::A, args: DispatchArgs) -> NodeConfigs<'static>;
    fn into_boxed(self) -> Box<dyn Domain<A = Self::A, T = Self::T>>
    where
        Self: Sized,
    {
        Box::new(self)
    }
}

impl<A: 'static, T: EmanationType> Domain for Box<dyn Domain<A = A, T = T>> {
    type A = A;
    type T = T;
    fn get_element(&self, kernel_context: Arc<KernelContext>) -> Element<Self::T> {
        self.as_ref().get_element(kernel_context)
    }
    fn dispatch_async(&self, domain_args: A, args: DispatchArgs) -> NodeConfigs<'static> {
        self.as_ref().dispatch_async(domain_args, args)
    }
    fn into_boxed(self) -> Box<dyn Domain<A = Self::A, T = Self::T>>
    where
        Self: Sized,
    {
        self
    }
}

pub struct DispatchArgs<'a> {
    pub call_kernel_async: &'a dyn Fn([u32; 3]) -> Command<'static, 'static>,
    pub debug_name: Option<String>,
}
