use std::marker::PhantomData;
use std::rc::Rc;

use dyn_clone::DynClone;
use luisa::runtime::{KernelArg, KernelParameter};

use crate::graph::NodeConfigs;
use crate::internal_prelude::*;
use crate::kernel::{ErasedKernelArg, ErasedKernelDispatch, KernelContext};
use crate::prelude::AsNodes;

pub mod dynamic;

pub trait PassthroughArg: KernelArg + 'static {}
impl<T: KernelArg + 'static> PassthroughArg for T {}

pub trait DomainImpl: Clone + Send + Sync + 'static {
    type Args: 'static;
    type Index: FieldIndex;
    type Passthrough: PassthroughArg;
    // TODO: perhaps should allow invoking the kernel multiple times, for example for the dual grid?
    fn get_element(
        &self,
        kernel_context: Rc<KernelContext>,
        passthrough: <Self::Passthrough as KernelArg>::Parameter,
    ) -> Element<Self::Index>;
    fn dispatch(
        &self,
        domain_args: Self::Args,
        args: KernelDispatch<Self::Passthrough>,
    ) -> NodeConfigs<'static>;
    // TODO: Consider making this take in an `Element` so that it's possible to implement dynamic contains using argument passing.
    fn contains_impl(&self, index: &Self::Index) -> Expr<bool>;
}

impl<X: DomainImpl> Domain for X
where
    X: DomainImpl,
{
    type Args = <X as DomainImpl>::Args;
    type Index = <X as DomainImpl>::Index;
    fn __get_element_erased(&self, kernel_context: Rc<KernelContext>) -> Element<Self::Index> {
        let passthrough = <X::Passthrough as KernelArg>::Parameter::def_param(
            &mut kernel_context.builder.borrow_mut(),
        );
        self.get_element(kernel_context, passthrough)
    }
    fn __dispatch_async_erased(
        &self,
        domain_args: Self::Args,
        args: ErasedKernelDispatch,
    ) -> NodeConfigs<'static> {
        self.dispatch(
            domain_args,
            KernelDispatch {
                erased: args,
                _marker: PhantomData,
            },
        )
    }
    fn contains(&self, index: &Self::Index) -> Expr<bool> {
        <Self as DomainImpl>::contains_impl(self, index)
    }
}

pub trait Domain: DynClone + Send + Sync + 'static {
    type Args: 'static;
    type Index: FieldIndex;
    fn contains(&self, index: &Self::Index) -> Expr<bool>;

    #[doc(hidden)]
    fn __get_element_erased(&self, kernel_context: Rc<KernelContext>) -> Element<Self::Index>;
    #[doc(hidden)]
    fn __dispatch_async_erased(
        &self,
        domain_args: Self::Args,
        args: ErasedKernelDispatch,
    ) -> NodeConfigs<'static>;
}
dyn_clone::clone_trait_object!(<A: 'static, I: FieldIndex> Domain<Args = A, Index = I>);

impl<A: 'static, I: FieldIndex> Domain for Box<dyn Domain<Args = A, Index = I>> {
    type Args = A;
    type Index = I;
    fn __get_element_erased(&self, kernel_context: Rc<KernelContext>) -> Element<I> {
        self.as_ref().__get_element_erased(kernel_context)
    }
    fn __dispatch_async_erased(
        &self,
        domain_args: A,
        args: ErasedKernelDispatch,
    ) -> NodeConfigs<'static> {
        self.as_ref().__dispatch_async_erased(domain_args, args)
    }
    fn contains(&self, index: &I) -> Expr<bool> {
        self.as_ref().contains(index)
    }
}

pub trait KernelDispatchT<P> {
    fn kernel_name(&self) -> Option<&str>;
    fn dispatch_raw(&self, dispatch_size: [u32; 3], passthrough: P) -> Command<'static, 'static>;
    fn dispatch(&self, dispatch_size: [u32; 3], passthrough: P) -> NodeConfigs<'static> {
        let config = self
            .dispatch_raw(dispatch_size, passthrough)
            .into_node_configs();
        if let Some(name) = self.kernel_name() {
            config.debug(name)
        } else {
            config
        }
    }
}

pub struct KernelDispatch<'a, P: PassthroughArg = ()> {
    erased: ErasedKernelDispatch<'a>,
    _marker: PhantomData<P>,
}
impl<P: PassthroughArg> KernelDispatch<'_, P> {
    pub fn kernel_name(&self) -> Option<&str> {
        self.erased.debug_name.as_deref()
    }
    pub fn dispatch_raw_with(
        &self,
        dispatch_size: [u32; 3],
        passthrough: P,
    ) -> Command<'static, 'static> {
        (self.erased.call_kernel_async)(
            dispatch_size,
            ErasedKernelArg {
                encode: Box::new(move |encoder| {
                    passthrough.encode(encoder);
                }),
            },
        )
    }
    pub fn dispatch_with(&self, dispatch_size: [u32; 3], passthrough: P) -> NodeConfigs<'static> {
        let config = self
            .dispatch_raw_with(dispatch_size, passthrough)
            .into_node_configs();
        if let Some(name) = self.kernel_name() {
            config.debug(name)
        } else {
            config
        }
    }
}
impl KernelDispatch<'_> {
    pub fn dispatch_raw(&self, dispatch_size: [u32; 3]) -> Command<'static, 'static> {
        self.dispatch_raw_with(dispatch_size, ())
    }
    pub fn dispatch(&self, dispatch_size: [u32; 3]) -> NodeConfigs<'static> {
        self.dispatch_with(dispatch_size, ())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NullDomain;
impl DomainImpl for NullDomain {
    type Args = ();
    type Index = ();
    type Passthrough = ();
    fn get_element(&self, kernel_context: Rc<KernelContext>, _: ()) -> Element<()> {
        Element::new((), Context::new(kernel_context))
    }
    fn dispatch(&self, _: (), _: KernelDispatch) -> NodeConfigs<'static> {
        ().into_node_configs()
    }
    fn contains_impl(&self, _: &()) -> Expr<bool> {
        false.expr()
    }
}
