use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Deref;
use std::rc::Rc;

use luisa::runtime::{AsKernelArg, KernelArg, KernelArgEncoder, KernelBuilder, KernelParameter};
use parking_lot::Mutex;
use pretty_type_name::pretty_type_name;

use crate::domain::{Domain, NullDomain};
use crate::graph::{ComputeGraph, NodeConfigs};
use crate::internal_prelude::*;

pub fn default_kernel_build_options() -> KernelBuildOptions {
    KernelBuildOptions {
        async_compile: true,
        ..Default::default()
    }
}

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
    pub(crate) builder: RefCell<KernelBuilder>,
    pub(crate) global_cache: RefCell<HashMap<FieldBinding, Box<dyn Any>>>,
}
impl KernelContext {
    pub fn bind_indirect<T: KernelArg, S: Deref<Target = T>>(
        &self,
        f: impl Fn() -> S + Send + 'static,
    ) -> T::Parameter {
        let mut builder = self.builder.borrow_mut();
        self.bindings.bindings.lock().push(Box::new(move |encoder| {
            f().encode(encoder);
        }));
        T::Parameter::def_param(&mut builder)
    }
    pub fn bind<T: KernelArg>(&self, f: impl Fn() -> T + Send + 'static) -> T::Parameter {
        let mut builder = self.builder.borrow_mut();
        self.bindings.bindings.lock().push(Box::new(move |encoder| {
            f().encode(encoder);
        }));
        T::Parameter::def_param(&mut builder)
    }
}

pub struct ErasedKernelArg {
    pub(crate) encode: Box<dyn Fn(&mut KernelArgEncoder)>,
}
pub struct PlaceholderKernelParam;

impl KernelArg for ErasedKernelArg {
    type Parameter = PlaceholderKernelParam;
    fn encode(&self, encoder: &mut KernelArgEncoder) {
        (self.encode)(encoder);
    }
}
impl AsKernelArg for ErasedKernelArg {
    type Output = Self;
}
impl KernelParameter for PlaceholderKernelParam {
    type Arg = ErasedKernelArg;
    fn def_param(_builder: &mut KernelBuilder) -> Self {
        // Parameter must be manually defined elsewhere.
        Self
    }
}

pub struct ErasedKernelDispatch<'a> {
    pub(crate) call_kernel_async:
        &'a dyn Fn([u32; 3], ErasedKernelArg) -> Command<'static, 'static>,
    pub(crate) debug_name: Option<String>,
}

trait KernelDomain: Send + Sync + 'static {
    type Args: 'static;
    fn kernel_dispatch_async(
        &self,
        domain_args: Self::Args,
        args: ErasedKernelDispatch,
    ) -> NodeConfigs<'static>;
}
impl<T> KernelDomain for T
where
    T: Domain,
{
    type Args = <T as Domain>::Args;
    fn kernel_dispatch_async(
        &self,
        domain_args: Self::Args,
        args: ErasedKernelDispatch,
    ) -> NodeConfigs<'static> {
        self.__dispatch_async_erased(domain_args, args)
    }
}

pub type LuisaKernel<S> = luisa::runtime::Kernel<<S as KernelSignature>::LuisaSignature>;

pub struct Kernel<S: KernelSignature, A: 'static = ()> {
    domain: Box<dyn KernelDomain<Args = A>>,
    pub(crate) raw: LuisaKernel<S>,
    pub(crate) bindings: KernelBindings,
    pub(crate) debug_name: Option<String>,
}
impl<S: KernelSignature, A: 'static> Debug for Kernel<S, A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&pretty_type_name::<Self>())
            .field("debug_name", &self.debug_name)
            .finish()
    }
}
impl Kernel<fn()> {
    pub fn null() -> Self {
        Self::build(&NullDomain, &|_| {})
    }
}
impl<S: KernelSignature, A: 'static> Kernel<S, A> {
    pub fn with_name(mut self, name: impl AsRef<str>) -> Self {
        self.debug_name = Some(name.as_ref().to_string());
        self
    }
    pub fn debug_name(&self) -> Option<&str> {
        self.debug_name.as_deref()
    }
    pub fn build_named<I: FieldIndex>(
        name: impl AsRef<str>,
        domain: &impl Domain<Index = I, Args = A>,
        f: S::Function<'_, I>,
    ) -> Self {
        Self::build_with_options(
            KernelBuildOptions {
                name: Some(name.as_ref().to_string()),
                ..default_kernel_build_options()
            },
            domain,
            f,
        )
        .with_name(name)
    }
    pub fn build<I: FieldIndex>(
        domain: &impl Domain<Index = I, Args = A>,
        f: S::Function<'_, I>,
    ) -> Self {
        Self::build_with_options(default_kernel_build_options(), domain, f)
    }
    pub fn build_with_options<I: FieldIndex>(
        options: KernelBuildOptions,
        domain: &impl Domain<Index = I, Args = A>,
        f: S::Function<'_, I>,
    ) -> Self {
        let domain = dyn_clone::clone(domain);
        let mut bindings = None;
        let mut builder = KernelBuilder::new(Some(DEVICE.clone()), true);
        let kernel = builder.build_kernel(|builder| {
            take_mut::take(builder, |builder| {
                let kernel_context = Rc::new(KernelContext {
                    bindings: KernelBindings::new(),
                    builder: RefCell::new(builder),
                    global_cache: RefCell::new(HashMap::new()),
                });

                let element = domain.__get_element_erased(kernel_context.clone());

                f.execute(element);

                let kernel_context = Rc::into_inner(kernel_context).unwrap();

                bindings = Some(kernel_context.bindings);
                kernel_context.builder.into_inner()
            });
        });
        let domain = Box::new(domain);
        // TODO: Fix the name - F is generally boring, or a closure inside so can grab the container name.
        Kernel {
            domain,
            raw: DEVICE.compile_kernel_def_with_options(&kernel, options),
            bindings: bindings.unwrap(),
            debug_name: None,
        }
    }
}
macro_rules! impl_kernel {
    () => {
        impl Kernel<fn()> {
            pub fn dispatch_blocking(&self) {
                self.dispatch_blocking_with(())
            }
            pub fn dispatch(&self) -> NodeConfigs<'static> {
                self.dispatch_with(())
            }
        }
        impl<A: 'static> Kernel<fn(), A> {
            pub fn dispatch_blocking_with(&self, domain_args: A) {
                let mut graph = ComputeGraph::new();
                graph.add(self.dispatch_with(domain_args));
                graph.execute();
            }
            pub fn dispatch_with(&self, domain_args: A) -> NodeConfigs<'static> {
                let args = ErasedKernelDispatch {
                    call_kernel_async: &|dispatch_size, arg| {
                        self.raw.dispatch_async(dispatch_size, &arg, &self.bindings)
                    },
                    debug_name: self.debug_name.clone(),
                };
                self.domain.kernel_dispatch_async(domain_args, args)
            }
        }
    };
    ($T0:ident: $S0:ident $(,$Tn:ident: $Sn:ident)*) => {
        impl<$T0: KernelArg + 'static $(, $Tn: KernelArg + 'static)*> Kernel<fn($T0 $(, $Tn)*)> {
            #[allow(non_snake_case)]
            #[allow(clippy::too_many_arguments)]
            pub fn dispatch_blocking<$S0: AsKernelArg<Output = $T0> $(, $Sn: AsKernelArg<Output = $Tn>)*>
                (&self, $S0: &$S0 $(, $Sn: &$Sn)*) {
                self.dispatch_blocking_with((), $S0 $(, $Sn)*)
            }
            #[allow(non_snake_case)]
            #[allow(clippy::too_many_arguments)]
            pub fn dispatch<$S0: AsKernelArg<Output = $T0> $(, $Sn: AsKernelArg<Output = $Tn>)*>
                (&self, $S0: &$S0 $(, $Sn: &$Sn)*) -> NodeConfigs<'static> {
                self.dispatch_with((), $S0 $(, $Sn)*)
            }
        }
        impl<A: 'static, $T0: KernelArg + 'static $(, $Tn: KernelArg + 'static)*> Kernel<fn($T0 $(, $Tn)*), A> {
            #[allow(non_snake_case)]
            #[allow(unused_variables)]
            #[allow(clippy::too_many_arguments)]
            pub fn dispatch_blocking_with<$S0: AsKernelArg<Output = $T0> $(, $Sn: AsKernelArg<Output = $Tn>)*>
                (&self, domain_args: A, $S0: &$S0 $(, $Sn: &$Sn)*) {
                let mut graph = ComputeGraph::new();
                graph.add(self.dispatch_with(domain_args, $S0 $(, $Sn)*));
                graph.execute();
            }
            #[allow(non_snake_case)]
            #[allow(unused_variables)]
            #[allow(clippy::too_many_arguments)]
            pub fn dispatch_with<$S0: AsKernelArg<Output = $T0> $(, $Sn: AsKernelArg<Output = $Tn>)*>
                (&self, domain_args: A, $S0: &$S0 $(, $Sn: &$Sn)*) -> NodeConfigs<'static> {
                let args = ErasedKernelDispatch {
                    call_kernel_async: &|dispatch_size, arg| {
                        self.raw.dispatch_async(dispatch_size, &arg, $S0, $($Sn,)* &self.bindings)
                    },
                    debug_name: self.debug_name.clone(),
                };
                self.domain.kernel_dispatch_async(domain_args, args)
            }
        }
        impl_kernel!( $($Tn: $Sn),* );
    };
}

impl_kernel!(T0:S0, T1:S1, T2:S2, T3:S3, T4:S4, T5:S5, T6:S6, T7:S7, T8:S8, T9:S9, T10:S10, T11:S11, T12:S12, T13:S13);

pub trait KernelSignature: Sized {
    // Adds `KernelContext` to the end of the signature.
    type LuisaSignature: luisa::runtime::KernelSignature;
    type Function<'a, T: FieldIndex>: KernelFunction<T, Self>;
}

macro_rules! impl_kernel_signature {
    () => {
        impl KernelSignature for fn() {
            type LuisaSignature = fn(ErasedKernelArg, KernelBindings);
            type Function<'a, I: FieldIndex> = &'a dyn Fn(Element<I>);
        }
    };
    ($T0:ident $(,$Tn:ident)*) => {
        impl<$T0: KernelArg + 'static $(,$Tn: KernelArg + 'static)*> KernelSignature for fn($T0 $(,$Tn)*) {
            type LuisaSignature = fn(ErasedKernelArg, $T0, $($Tn,)* KernelBindings);
            type Function<'a, I: FieldIndex> = &'a dyn Fn(Element<I>, <$T0 as KernelArg>::Parameter $(,<$Tn as KernelArg>::Parameter)*);
        }
        impl_kernel_signature!($($Tn),*);
    };
}

impl_kernel_signature!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);

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
                let kernel_context = el.context().kernel.clone();
                let mut builder = kernel_context.builder.borrow_mut();
                let $T0 = <$T0::Parameter as KernelParameter>::def_param(&mut builder);
                $(let $Tn = <$Tn::Parameter as KernelParameter>::def_param(&mut builder);)*
                drop(builder);

                (self)(el, $T0 $(,$Tn)*)
            }
        }
        impl_kernel_function!($($Tn),*);
    }
}

impl_kernel_function!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
