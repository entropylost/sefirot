use std::sync::Arc;

use luisa::runtime::{AsKernelArg, KernelArg, KernelArgEncoder, KernelBuilder, KernelParameter};
use parking_lot::Mutex;

use crate::domain::{DispatchArgs, Domain};
use crate::graph::{ComputeGraph, NodeConfigs};
use crate::internal_prelude::*;

#[derive(Default)]
pub struct KernelBindings {
    bindings: Mutex<Vec<Box<dyn Fn(&mut KernelArgEncoder) + Send>>>,
}
impl KernelBindings {
    pub fn new() -> Self {
        Self::default()
    }
}

impl KernelArg for KernelBindings {
    type Parameter = ();
    fn encode(&self, encoder: &mut KernelArgEncoder) {
        for binding in self.bindings.lock().iter() {
            binding(encoder);
        }
    }
}
impl AsKernelArg for KernelBindings {
    type Output = Self;
}

pub struct KernelContext {
    bindings: KernelBindings,
    builder: Mutex<KernelBuilder>,
}

pub type LuisaKernel<S> = luisa::runtime::Kernel<<S as KernelSignature>::LuisaSignature>;

// TODO: Find a way of passing the domain into the kernel.
pub struct Kernel<I: FieldIndex, S: KernelSignature, A: 'static = ()> {
    pub(crate) domain: Box<dyn Domain<I = I, A = A>>,
    pub(crate) raw: LuisaKernel<S>,
    pub(crate) bindings: KernelBindings,
    pub(crate) debug_name: Option<String>,
    pub(crate) device: Device,
}
impl<I: FieldIndex, S: KernelSignature, A: 'static> Kernel<I, S, A> {
    pub fn with_name(mut self, name: impl AsRef<str>) -> Self {
        self.debug_name = Some(name.as_ref().to_string());
        self
    }
    pub fn debug_name(&self) -> Option<&str> {
        self.debug_name.as_deref()
    }
    pub fn build(
        device: &Device,
        domain: impl Domain<I = I, A = A>,
        f: S::Function<'_, I>,
    ) -> Self {
        Self::build_with_options(
            device,
            KernelBuildOptions {
                async_compile: true,
                ..Default::default()
            },
            domain,
            f,
        )
    }
    pub fn build_with_options(
        device: &Device,
        options: KernelBuildOptions,
        domain: impl Domain<I = I, A = A>,
        f: S::Function<'_, I>,
    ) -> Self {
        let domain = domain.into_boxed();
        let mut bindings = None;
        let mut builder = KernelBuilder::new(Some(device.clone()), true);
        let kernel = builder.build_kernel(|builder| {
            take_mut::take(builder, |builder| {
                let kernel_context = Arc::new(KernelContext {
                    bindings: KernelBindings::new(),
                    builder: Mutex::new(builder),
                });

                let element = domain.get_element(kernel_context.clone());

                f.execute(element);

                let kernel_context = Arc::into_inner(kernel_context).unwrap();

                bindings = Some(kernel_context.bindings);
                kernel_context.builder.into_inner()
            });
        });
        // TODO: Fix the name - F is generally boring. Perhaps with `CoerceUnsized`?
        Kernel {
            domain,
            raw: device.compile_kernel_def_with_options(&kernel, options),
            bindings: bindings.unwrap(),
            debug_name: None,
            device: device.clone(),
        }
    }
}
macro_rules! impl_kernel {
    () => {
        impl<I: FieldIndex> Kernel<I, fn()> {
            pub fn dispatch_blocking(&self) {
                self.dispatch_blocking_with_domain_args(())
            }
            pub fn dispatch(&self) -> NodeConfigs<'static> {
                self.dispatch_with_domain_args(())
            }
        }
        impl<I: FieldIndex, A: 'static> Kernel<I, fn(), A> {
            pub fn dispatch_blocking_with_domain_args(&self, domain_args: A) {
                let mut graph = ComputeGraph::new(&self.device);
                graph.add(self.dispatch_with_domain_args(domain_args));
                graph.execute();
            }
            pub fn dispatch_with_domain_args(&self, domain_args: A) -> NodeConfigs<'static> {
                let args = DispatchArgs {
                    call_kernel_async: &|dispatch_size| {
                        self.raw.dispatch_async(dispatch_size, &self.bindings)
                    },
                    debug_name: self.debug_name.clone(),
                };
                self.domain.dispatch_async(domain_args, args)
            }
        }
    };
    ($T0:ident: $S0:ident $(,$Tn:ident: $Sn:ident)*) => {
        impl<I: FieldIndex, $T0: KernelArg + 'static $(, $Tn: KernelArg + 'static)*> Kernel<I, fn($T0 $(, $Tn)*)> {
            #[allow(non_snake_case)]
            #[allow(clippy::too_many_arguments)]
            pub fn dispatch_blocking<$S0: AsKernelArg<Output = $T0> $(, $Sn: AsKernelArg<Output = $Tn>)*>
                (&self, $S0: &$S0 $(, $Sn: &$Sn)*) {
                self.dispatch_blocking_with_domain_args((), $S0 $(, $Sn)*)
            }
            #[allow(non_snake_case)]
            #[allow(clippy::too_many_arguments)]
            pub fn dispatch<$S0: AsKernelArg<Output = $T0> $(, $Sn: AsKernelArg<Output = $Tn>)*>
                (&self, $S0: &$S0 $(, $Sn: &$Sn)*) -> NodeConfigs<'static> {
                self.dispatch_with_domain_args((), $S0 $(, $Sn)*)
            }
        }
        impl<I: FieldIndex, A: 'static, $T0: KernelArg + 'static $(, $Tn: KernelArg + 'static)*> Kernel<I, fn($T0 $(, $Tn)*), A> {
            #[allow(non_snake_case)]
            #[allow(unused_variables)]
            #[allow(clippy::too_many_arguments)]
            pub fn dispatch_blocking_with_domain_args<$S0: AsKernelArg<Output = $T0> $(, $Sn: AsKernelArg<Output = $Tn>)*>
                (&self, domain_args: A, $S0: &$S0 $(, $Sn: &$Sn)*) {
                let mut graph = ComputeGraph::new(&self.device);
                graph.add(self.dispatch_with_domain_args(domain_args, $S0 $(, $Sn)*));
                graph.execute();
            }
            #[allow(non_snake_case)]
            #[allow(unused_variables)]
            #[allow(clippy::too_many_arguments)]
            pub fn dispatch_with_domain_args<$S0: AsKernelArg<Output = $T0> $(, $Sn: AsKernelArg<Output = $Tn>)*>
                (&self, domain_args: A, $S0: &$S0 $(, $Sn: &$Sn)*) -> NodeConfigs<'static> {
                let args = DispatchArgs {
                    call_kernel_async: &|dispatch_size| {
                        self.raw.dispatch_async(dispatch_size, $S0, $($Sn,)* &self.bindings)
                    },
                    debug_name: self.debug_name.clone(),
                };
                self.domain.dispatch_async(domain_args, args)
            }
        }
        impl_kernel!( $($Tn: $Sn),* );
    };
}

impl_kernel!(T0:S0, T1:S1, T2:S2, T3:S3, T4:S4, T5:S5, T6:S6, T7:S7, T8:S8, T9:S9, T10:S10, T11:S11, T12:S12, T13:S13, T14:S14);

pub trait KernelSignature: Sized {
    // Adds `KernelContext` to the end of the signature.
    type LuisaSignature: luisa::runtime::KernelSignature;
    type Function<'a, T: FieldIndex>: KernelFunction<T, Self>;
}

macro_rules! impl_kernel_signature {
    () => {
        impl KernelSignature for fn() {
            type LuisaSignature = fn(KernelBindings);
            type Function<'a, I: FieldIndex> = &'a dyn Fn(Element<I>);
        }
    };
    ($T0:ident $(,$Tn:ident)*) => {
        impl<$T0: KernelArg + 'static $(,$Tn: KernelArg + 'static)*> KernelSignature for fn($T0 $(,$Tn)*) {
            type LuisaSignature = fn($T0, $($Tn,)* KernelBindings);
            type Function<'a, I: FieldIndex> = &'a dyn Fn(Element<I>, <$T0 as KernelArg>::Parameter $(,<$Tn as KernelArg>::Parameter)*);
        }
        impl_kernel_signature!($($Tn),*);
    };
}

impl_kernel_signature!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);

pub trait KernelFunction<I: FieldIndex, S: KernelSignature> {
    fn execute(&self, el: Element<I>);
}

macro_rules! impl_kernel_function {
    () => {
        impl<I: FieldIndex> KernelFunction<I, fn()> for &dyn Fn(Element<I>) {
            fn execute(&self, el: Element<I>) {
                self(el);
            }
        }
    };
    ($T0:ident $(,$Tn:ident)*) => {
        impl<I: FieldIndex, $T0: KernelArg + 'static $(,$Tn: KernelArg + 'static)*> KernelFunction<I, fn($T0 $(,$Tn)*)> for
            &dyn Fn(Element<I>, $T0::Parameter $(,$Tn::Parameter)*)
        {
            #[allow(non_snake_case)]
            #[allow(unused_variables)]
            fn execute(&self, el: Element<I>) {
                let mut builder = el.context.kernel.builder.lock();
                let $T0 = <$T0::Parameter as KernelParameter>::def_param(&mut builder);
                $(let $Tn = <$Tn::Parameter as KernelParameter>::def_param(&mut builder);)*
                drop(builder);

                (self)(el, $T0 $(,$Tn)*)
            }
        }
        impl_kernel_function!($($Tn),*);
    }
}

impl_kernel_function!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
