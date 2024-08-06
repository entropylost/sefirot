use std::rc::Rc;
use std::sync::Arc;

use luisa::lang::types::vector::Vec3;
use parking_lot::Mutex;

use super::{DomainImpl, KernelDispatch};
use crate::graph::NodeConfigs;
use crate::internal_prelude::*;
use crate::kernel::KernelContext;

#[derive(Debug, Clone)]
pub struct DynamicDomain {
    pub len: Arc<Mutex<u32>>,
}

// TODO: Also allow using dispatch args. Also 2d, 3d versions.
impl DynamicDomain {
    pub fn new(len: u32) -> Self {
        DynamicDomain {
            len: Arc::new(Mutex::new(len)),
        }
    }
}

impl DomainImpl for DynamicDomain {
    type Args = ();
    type Index = Expr<u32>;
    type Passthrough = ();
    fn get_element(&self, kernel_context: Rc<KernelContext>, _: ()) -> Element<Self::Index> {
        Element::new(dispatch_id().x, Context::new(kernel_context))
    }
    fn dispatch(&self, _: (), args: KernelDispatch) -> NodeConfigs<'static> {
        args.dispatch([*self.len.lock(), 1, 1])
    }
    fn contains_impl(&self, _el: &Element<Self::Index>) -> Expr<bool> {
        // TODO: Can use ConstantAccessor here.
        unimplemented!("Cannot check if an index is contained in the domain");
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SizedDomain;

impl DomainImpl for SizedDomain {
    type Args = [u32; 3];
    type Index = Expr<Vec3<u32>>;
    type Passthrough = ();
    fn get_element(&self, kernel_context: Rc<KernelContext>, _: ()) -> Element<Self::Index> {
        Element::new(dispatch_id(), Context::new(kernel_context))
    }
    fn dispatch(&self, args: [u32; 3], dispatch: KernelDispatch) -> NodeConfigs<'static> {
        dispatch.dispatch(args)
    }
    fn contains_impl(&self, _el: &Element<Self::Index>) -> Expr<bool> {
        unimplemented!("Cannot check if an index is contained in the domain");
    }
}
