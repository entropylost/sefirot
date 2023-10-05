use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use luisa::runtime::KernelBuilder;

use crate::element::{Context, KernelContext};
use crate::graph::{CommandNode, NodeData};
use crate::prelude::*;

pub struct Kernel<D: Domain<T>, T: EmanationType> {
    domain: D,
    raw: luisa::runtime::Kernel<fn(Context)>,
    context: Arc<Context>,
    debug_name: Option<String>,
    _marker: PhantomData<T>,
}
impl<D: Domain<T>, T: EmanationType> Kernel<D, T> {
    pub fn with_name(mut self, name: impl AsRef<str>) -> Self {
        self.debug_name = Some(name.as_ref().to_owned());
        self
    }
}

pub trait IndexEmanation<I, T: EmanationType> {
    fn bind_fields(&self, idx: I, element: &mut Element<T>);
}
impl<T: EmanationType> Emanation<T> {
    pub fn get<'a, S: EmanationType, I, Idx: IndexEmanation<I, T>>(
        &'a self,
        context: &'a KernelContext<'a>,
        indexer: Idx,
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
    // Store domain with kernel.
    pub fn build_kernel<D: Domain<T>>(
        &self,
        device: &Device,
        domain: D,
        f: impl FnOnce(Element<T>),
    ) -> Kernel<D, T> {
        let context = Context::new();
        let mut builder = KernelBuilder::new(Some(device.clone()), true);
        let kernel = builder.build_kernel(|builder| {
            let context = KernelContext {
                context: &context,
                builder: Mutex::new(builder),
            };

            let mut element = Element {
                emanation: self,
                overridden_accessors: HashMap::new(),
                context: &context,
                cache: HashMap::new(),
                unsaved_fields: HashSet::new(),
                can_write: true,
            };
            domain.before_record(&mut element);
            f(element);
        });
        Kernel {
            domain,
            raw: device.compile_kernel_def(&kernel),
            context: Arc::new(context),
            debug_name: None,
            _marker: PhantomData,
        }
    }
}

pub trait IndexDomain<T: EmanationType>: IndexEmanation<Self::I, T> {
    type I;
    fn get_index(&self) -> Self::I;
    fn dispatch_size(&self) -> [u32; 3];
}

impl<X, T: EmanationType> Domain<T> for X
where
    X: IndexDomain<T>,
{
    fn before_record(&self, element: &mut Element<T>) {
        let index = self.get_index();
        self.bind_fields(index, element);
    }
    fn dispatch(kernel: &Kernel<Self, T>) {
        let dispatch_size = kernel.domain.dispatch_size();
        kernel.raw.dispatch(dispatch_size, &*kernel.context);
    }
    fn dispatch_async(kernel: &Kernel<Self, T>) -> NodeData<'static> {
        let dispatch_size = kernel.domain.dispatch_size();
        NodeData::Command(CommandNode {
            context: kernel.context.clone(),
            command: kernel.raw.dispatch_async(dispatch_size, &*kernel.context),
            debug_name: kernel.debug_name.clone(),
        })
    }
}

pub trait Domain<T: EmanationType>: Sized {
    fn before_record(&self, element: &mut Element<T>);
    fn dispatch(kernel: &Kernel<Self, T>);
    fn dispatch_async(kernel: &Kernel<Self, T>) -> NodeData<'static>;
}
