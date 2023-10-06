use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use luisa::runtime::KernelBuilder;

use pretty_type_name::pretty_type_name;

use crate::element::{Context, KernelContext};
use crate::graph::{AddToComputeGraph, CommandNode, ComputeGraph, NodeData, NodeHandle};
use crate::prelude::*;

mod kernel;
use kernel::Kernel;

use self::kernel::{KernelArgs, KernelSignature};

pub trait IndexEmanation<I> {
    type T: EmanationType;
    fn bind_fields(&self, idx: I, element: &mut Element<Self::T>);
}
impl<T: EmanationType> Emanation<T> {
    pub fn get<'a, S: EmanationType, I, Idx: IndexEmanation<I, T = T>>(
        &'a self,
        context: &'a KernelContext<'a>,
        indexer: &Idx,
        idx: I,
    ) -> Element<'a, T> {
        let mut element = Element {
            emanation: self,
            overridden_accessors: HashMap::new(),
            context,
            cache: HashMap::new(),
            unsaved_fields: HashSet::new(),
            can_write: true,
        };
        indexer.bind_fields(idx, &mut element);
        element
    }
}

pub trait IndexDomain: IndexEmanation<Self::I> {
    type I;
    fn get_index(&self) -> Self::I;
    fn dispatch_size(&self) -> [u32; 3];
}

impl<X> Domain for X
where
    X: IndexDomain,
{
    type T = X::T;
    fn before_record(&self, element: &mut Element<X::T>) {
        let index = self.get_index();
        self.bind_fields(index, element);
    }
    fn dispatch<S: KernelSignature>(kernel: &Kernel<Self, S>, args: impl KernelArgs<S = S>) {
        let dispatch_size = kernel.domain.dispatch_size();
        KernelArgs::dispatch_with_size(&kernel.raw, dispatch_size, &*kernel.context, args);
    }
    fn dispatch_async<S: KernelSignature>(
        kernel: &Kernel<Self, S>,
        graph: &mut ComputeGraph,
        args: impl KernelArgs<S = S>,
    ) -> NodeHandle {
        let dispatch_size = kernel.domain.dispatch_size();
        graph.add(NodeData::Command(CommandNode {
            context: kernel.context.clone(),
            command: KernelArgs::dispatch_with_size_async(
                &kernel.raw,
                dispatch_size,
                &*kernel.context,
                args,
            ),
            debug_name: kernel.debug_name.clone(),
        }))
    }
}

pub trait Domain: Sized {
    type T: EmanationType;
    fn before_record(&self, element: &mut Element<Self::T>);
    fn dispatch<S: KernelSignature>(kernel: &Kernel<Self, S>, args: impl KernelArgs<S = S>);
    fn dispatch_async<'a, S: KernelSignature>(
        kernel: &Kernel<Self, S>,
        graph: &mut ComputeGraph<'a>,
        args: impl KernelArgs<S = S>,
    ) -> NodeHandle;
}
