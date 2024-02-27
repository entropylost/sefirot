use std::marker::PhantomData;
use std::sync::Arc;

use parking_lot::Mutex;

use luisa::runtime::KernelBuilder;

use crate::element::{Context, KernelContext};
use crate::graph::{AsNodes, ComputeGraph, NodeConfigs};
use crate::prelude::*;

pub mod kernel;

/// A trait for types that can be used to generate new [`Element`]s within a kernel,
/// given an index of type `I` (see [`Emanation::get`]).
pub trait IndexEmanation<I> {
    type T: EmanationType;
    /// Bind the fields necessary to make the [`Element`] valid.
    /// For example, [`ArrayIndex`] binds the `EField<u32, T>`'s value to the index.
    fn bind_fields(&self, index: I, element: &Element<Self::T>);
}
impl<I, X> IndexEmanation<I> for &X
where
    X: IndexEmanation<I>,
{
    type T = X::T;
    fn bind_fields(&self, index: I, element: &Element<Self::T>) {
        (*self).bind_fields(index, element);
    }
}
impl<T: EmanationType> Emanation<T> {
    pub fn get<I, Idx: IndexEmanation<I, T = T>>(
        &self,
        context: &KernelContext,
        indexer: &Idx,
        idx: I,
    ) -> Element<T> {
        let element = Element::new(self.clone(), context.clone());
        indexer.bind_fields(idx, &element);
        element
    }
}

/// A trait for simple [`Domain`]s generated from a single 3-dimensional dispatch call.
pub trait IndexDomain: IndexEmanation<Self::I> + 'static + Send + Sync {
    type I;
    type A;
    fn get_index(&self) -> Self::I;
    fn dispatch_size(&self, args: Self::A) -> [u32; 3];
    fn before_dispatch(&self, _args: &Self::A) {}
}
impl<X> Domain for X
where
    X: IndexDomain,
{
    type T = X::T;
    type A = X::A;
    fn before_record(&self, element: &Element<X::T>) {
        let index = self.get_index();
        self.bind_fields(index, element);
    }
    fn dispatch_async(&self, domain_args: X::A, args: DispatchArgs) -> NodeConfigs<'static> {
        self.before_dispatch(&domain_args);
        let dispatch_size = self.dispatch_size(domain_args);
        ((args.call_kernel_async)(dispatch_size))
            .debug(args.debug_name.unwrap_or("kernel call".to_string()))
    }
}

/// A trait representing a space across which computations may be performed by calling kernels.
/// This is intentionally very generic, and does not provide any guarantees on how many dispatch calls are generated.
/// For most purposes, [`IndexDomain`] is a conveinent way to implement this trait if only a single dispatch call is necessary.
pub trait Domain: Send + Sync + 'static {
    type T: EmanationType;
    type A;
    fn before_record(&self, element: &Element<Self::T>);
    fn dispatch_async(&self, domain_args: Self::A, args: DispatchArgs) -> NodeConfigs<'static>;
}

pub trait AsBoxedDomain {
    type T: EmanationType;
    type A;
    fn into_boxed_domain(self) -> Box<dyn Domain<T = Self::T, A = Self::A>>;
}
impl<T: EmanationType, A> AsBoxedDomain for Box<dyn Domain<T = T, A = A>> {
    type T = T;
    type A = A;
    fn into_boxed_domain(self) -> Box<dyn Domain<T = T, A = A>> {
        self
    }
}
impl<T: EmanationType, A, D> AsBoxedDomain for D
where
    D: Domain<T = T, A = A>,
{
    type T = T;
    type A = A;
    fn into_boxed_domain(self) -> Box<dyn Domain<T = T, A = A>> {
        Box::new(self)
    }
}

pub struct DispatchArgs<'a> {
    pub context: Arc<Context>,
    pub call_kernel_async: &'a dyn Fn([u32; 3]) -> Command<'static, 'static>,
    pub debug_name: Option<String>,
}

pub trait DomainExt: Domain + Sized {
    fn map<B, F: Fn(B) -> Self::A + Send + Sync + 'static>(self, f: F) -> MappedDomain<Self, B, F> {
        MappedDomain {
            domain: self,
            f,
            _marker: PhantomData,
        }
    }
}
impl<X: Domain + Sized> DomainExt for X {}

pub struct MappedDomain<D: Domain, B, F: Fn(B) -> D::A + Send + Sync> {
    domain: D,
    f: F,
    _marker: PhantomData<fn(B)>,
}
impl<D: Domain, B: 'static, F: Fn(B) -> D::A + Send + Sync + 'static> Domain
    for MappedDomain<D, B, F>
{
    type T = D::T;
    type A = B;
    fn before_record(&self, element: &Element<Self::T>) {
        self.domain.before_record(element);
    }
    fn dispatch_async(&self, domain_args: B, args: DispatchArgs) -> NodeConfigs<'static> {
        self.domain.dispatch_async((self.f)(domain_args), args)
    }
}
