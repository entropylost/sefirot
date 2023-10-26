use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::Mutex;

use luisa::runtime::KernelBuilder;

use pretty_type_name::pretty_type_name;

use crate::element::{Context, KernelContext};
use crate::graph::{AddToComputeGraph, CommandNode, ComputeGraph, NodeData, NodeHandle};
use crate::prelude::*;

pub mod kernel;

pub trait IndexEmanation<I> {
    type T: EmanationType;
    fn bind_fields(&self, index: I, element: &Element<Self::T>);
}
impl<T: EmanationType> Emanation<T> {
    pub fn get<I, Idx: IndexEmanation<I, T = T>>(
        &self,
        context: &KernelContext,
        indexer: &Idx,
        idx: I,
    ) -> Element<T> {
        let element = Element {
            emanation: self.clone(),
            overridden_accessors: Mutex::new(HashMap::new()),
            context: context.clone(),
            cache: Mutex::new(HashMap::new()),
            unsaved_fields: Mutex::new(HashSet::new()),
        };
        indexer.bind_fields(idx, &element);
        element
    }
}

pub trait IndexDomain: IndexEmanation<Self::I> {
    type I;
    type A;
    fn get_index(&self) -> Self::I;
    fn dispatch_size(&self, args: Self::A) -> [u32; 3];
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
    fn dispatch(&self, domain_args: X::A, args: DispatchArgs) {
        let dispatch_size = self.dispatch_size(domain_args);
        (args.call_kernel)(dispatch_size);
    }
    fn dispatch_async(
        &self,
        graph: &mut ComputeGraph<'_>,
        domain_args: X::A,
        args: DispatchArgs,
    ) -> NodeHandle {
        let dispatch_size = self.dispatch_size(domain_args);
        *graph.add(NodeData::Command(CommandNode {
            context: args.context.clone(),
            command: (args.call_kernel_async)(dispatch_size),
            debug_name: args.debug_name.clone(),
        }))
    }
}

pub trait Domain {
    type T: EmanationType;
    type A;
    fn before_record(&self, element: &Element<Self::T>);
    fn dispatch(&self, domain_args: Self::A, args: DispatchArgs);
    fn dispatch_async(
        &self,
        graph: &mut ComputeGraph<'_>,
        domain_args: Self::A,
        args: DispatchArgs,
    ) -> NodeHandle;
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
impl<T: EmanationType, A, D: Domain<T = T, A = A> + 'static> AsBoxedDomain for D {
    type T = T;
    type A = A;
    fn into_boxed_domain(self) -> Box<dyn Domain<T = T, A = A>> {
        Box::new(self)
    }
}

pub struct DispatchArgs<'a> {
    pub context: Arc<Context>,
    pub call_kernel: &'a dyn Fn([u32; 3]),
    pub call_kernel_async: &'a dyn Fn([u32; 3]) -> Command<'static, 'static>,
    pub debug_name: Option<String>,
}
