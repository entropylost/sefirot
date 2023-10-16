use luisa::runtime::{AsKernelArg, KernelArg, KernelParameter};

use super::*;

pub type LuisaKernel<S> = luisa::runtime::Kernel<<S as KernelSignature>::LuisaSignature>;

pub struct Kernel<T: EmanationType, S: KernelSignature> {
    pub(crate) domain: Box<dyn Domain<T = T>>,
    pub(crate) raw: LuisaKernel<S>,
    pub(crate) context: Arc<Context>,
    pub(crate) debug_name: Option<String>,
}
impl<T: EmanationType, S: KernelSignature> Kernel<T, S> {
    pub fn with_name(mut self, name: impl AsRef<str>) -> Self {
        self.debug_name = Some(name.as_ref().to_string());
        self
    }
}

impl<T: EmanationType> Emanation<T> {
    pub fn build_kernel<F: KernelSignature>(
        &self,
        domain: impl IntoBoxedDomain<T = T>,
        f: F::Function<'_, T>,
    ) -> Kernel<T, F> {
        self.build_kernel_with_options(Default::default(), domain, f)
    }

    pub fn build_kernel_with_options<F: KernelSignature>(
        &self,
        options: KernelBuildOptions,
        domain: impl IntoBoxedDomain<T = T>,
        f: F::Function<'_, T>,
    ) -> Kernel<T, F> {
        let domain = domain.into_boxed_domain();
        let context = Arc::new(Context::new());
        let mut builder = KernelBuilder::new(Some(self.device.clone()), true);
        let kernel = builder.build_kernel(|builder| {
            take_mut::take(builder, |builder| {
                let context = KernelContext {
                    context: context.clone(),
                    builder: Arc::new(Mutex::new(builder)),
                };
                let builder = context.builder.clone();

                let element = Element {
                    emanation: self.clone(),
                    overridden_accessors: Mutex::new(HashMap::new()),
                    context,
                    cache: Mutex::new(HashMap::new()),
                    unsaved_fields: Mutex::new(HashSet::new()),
                };
                domain.before_record(&element);
                f.execute(element);
                Arc::into_inner(builder).unwrap().into_inner()
            });
        });
        let name = pretty_type_name::<F>();
        Kernel {
            domain,
            raw: self
                .device
                .compile_kernel_def_with_options(&kernel, options),
            context,
            debug_name: Some(name),
        }
    }
}

macro_rules! impl_kernel {
    () => {
        impl<T: EmanationType> Kernel<T, fn()> {
            pub fn dispatch_blocking(&self) {
                let args = DispatchArgs {
                    context: self.context.clone(),
                    call_kernel: &|dispatch_size| self.raw.dispatch(dispatch_size, &*self.context),
                    call_kernel_async: &|dispatch_size| {
                        self.raw.dispatch_async(dispatch_size, &*self.context)
                    },
                    debug_name: self.debug_name.clone(),
                };
                self.domain.dispatch(args);
            }
            pub fn dispatch<'a: 'b, 'b>(&'b self) -> impl AddToComputeGraph<'a> + 'b {
                let context = self.context.clone();

                move |graph: &mut ComputeGraph<'a>| {
                    let args = DispatchArgs {
                        context: self.context.clone(),
                        call_kernel: &move |dispatch_size| self.raw.dispatch(dispatch_size, &*context),
                        call_kernel_async: &|dispatch_size| {
                            self.raw.dispatch_async(dispatch_size, &*self.context)
                        },
                        debug_name: self.debug_name.clone(),
                    };
                    self.domain.dispatch_async(graph, args)
                }
            }
        }
    };
    ($T0:ident: $S0:ident $(,$Tn:ident: $Sn:ident)*) => {
        impl<T: EmanationType, $T0: KernelArg + 'static $(, $Tn: KernelArg + 'static)*> Kernel<T, fn($T0 $(, $Tn)*)> {
            #[allow(non_snake_case)]
            #[allow(unused_variables)]
            #[allow(clippy::too_many_arguments)]
            pub fn dispatch_blocking<$S0: AsKernelArg<Output = $T0> $(, $Sn: AsKernelArg<Output = $Tn>)*>(&self, $S0: &$S0 $(, $Sn: &$Sn)*) {
                let args = DispatchArgs {
                    context: self.context.clone(),
                    call_kernel: &|dispatch_size| self.raw.dispatch(dispatch_size, $S0, $($Sn,)* &*self.context),
                    call_kernel_async: &|dispatch_size| {
                        self.raw.dispatch_async(dispatch_size, $S0, $($Sn,)* &*self.context)
                    },
                    debug_name: self.debug_name.clone(),
                };
                self.domain.dispatch(args);
            }
            #[allow(non_snake_case)]
            #[allow(unused_variables)]
            #[allow(clippy::too_many_arguments)]
            pub fn dispatch<'a: 'b, 'b, $S0: AsKernelArg<Output = $T0> $(, $Sn: AsKernelArg<Output = $Tn>)*>
                (&'b self, $S0: &'b $S0 $(, $Sn: &'b $Sn)*) -> impl AddToComputeGraph<'a> + 'b {
                let context = self.context.clone();
                move |graph: &mut ComputeGraph<'a>| {
                    let args = DispatchArgs {
                        context: self.context.clone(),
                        call_kernel: &move |dispatch_size| self.raw.dispatch(dispatch_size, $S0, $($Sn,)* &*context),
                        call_kernel_async: &|dispatch_size| {
                            self.raw.dispatch_async(dispatch_size, $S0, $($Sn,)* &*self.context)
                        },
                        debug_name: self.debug_name.clone(),
                    };
                    self.domain.dispatch_async(graph, args)
                }
            }
        }
        impl_kernel!( $($Tn: $Sn),* );
    };
}

impl_kernel!(T0:S0, T1:S1, T2:S2, T3:S3, T4:S4, T5:S5, T6:S6, T7:S7, T8:S8, T9:S9, T10:S10, T11:S11, T12:S12, T13:S13, T14:S14);

pub trait KernelSignature: Sized {
    // Adds `Context` to the end of the signature.
    type LuisaSignature: luisa::runtime::KernelSignature;
    type Function<'a, T: EmanationType>: KernelFunction<T, Self>;
}

macro_rules! impl_kernel_signature {
    () => {
        impl KernelSignature for fn() {
            type LuisaSignature = fn(Context);
            type Function<'a, T: EmanationType> = &'a dyn Fn(&Element<T>);
        }
    };
    ($T0:ident $(,$Tn:ident)*) => {
        impl<$T0: KernelArg + 'static $(,$Tn: KernelArg + 'static)*> KernelSignature for fn($T0 $(,$Tn)*) {
            type LuisaSignature = fn($T0, $($Tn,)* Context);
            type Function<'a, T: EmanationType> = &'a dyn Fn(&Element<T>, <$T0 as KernelArg>::Parameter $(,<$Tn as KernelArg>::Parameter)*);
        }
        impl_kernel_signature!($($Tn),*);
    };
}

impl_kernel_signature!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);

pub trait KernelFunction<T: EmanationType, S: KernelSignature> {
    fn execute(&self, el: Element<T>);
}

macro_rules! impl_kernel_function {
    () => {
        impl<T: EmanationType> KernelFunction<T, fn()> for &dyn Fn(&Element<T>) {
            fn execute(&self, el: Element<T>) {
                self(&el);
            }
        }
    };
    ($T0:ident $(,$Tn:ident)*) => {
        impl<T: EmanationType, $T0: KernelArg + 'static $(,$Tn: KernelArg + 'static)*> KernelFunction<T, fn($T0 $(,$Tn)*)> for
            &dyn Fn(&Element<T>, $T0::Parameter $(,$Tn::Parameter)*)
        {
            #[allow(non_snake_case)]
            #[allow(unused_variables)]
            fn execute(&self, el: Element<T>) {
                let mut builder = el.context.builder.lock();
                let $T0 = <$T0::Parameter as KernelParameter>::def_param(&mut builder);
                $(let $Tn = <$Tn::Parameter as KernelParameter>::def_param(&mut builder);)*

                (self)(&el, $T0 $(,$Tn)*)
            }
        }
        impl_kernel_function!($($Tn),*);
    }
}

impl_kernel_function!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
