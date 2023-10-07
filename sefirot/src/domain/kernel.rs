use luisa::runtime::{AsKernelArg, KernelArg, KernelParameter};

use super::*;

pub type LuisaKernel<S> = luisa::runtime::Kernel<<S as KernelSignature>::LuisaSignature>;

pub struct Kernel<D: Domain, S: KernelSignature> {
    pub(crate) domain: D,
    pub(crate) raw: LuisaKernel<S>,
    pub(crate) context: Arc<Context>,
    pub(crate) debug_name: Option<String>,
}
impl<D: Domain, S: KernelSignature> Kernel<D, S> {
    pub fn with_name(mut self, name: impl AsRef<str>) -> Self {
        self.debug_name = Some(name.as_ref().to_owned());
        self
    }
}

impl<T: EmanationType> Emanation<T> {
    pub fn build_kernel<D: Domain<T = T>, F: KernelSignature>(
        &self,
        device: &Device,
        domain: D,
        f: F::Function<'_, T>,
    ) -> Kernel<D, F> {
        self.build_kernel_with_options(device, Default::default(), domain, f)
    }

    pub fn build_kernel_with_options<D: Domain<T = T>, F: KernelSignature>(
        &self,
        device: &Device,
        options: KernelBuildOptions,
        domain: D,
        f: F::Function<'_, T>,
    ) -> Kernel<D, F> {
        let context = Context::new();
        let mut builder = KernelBuilder::new(Some(device.clone()), true);
        let kernel = builder.build_kernel(|builder| {
            let context = KernelContext {
                context: &context,
                builder: Mutex::new(builder),
            };

            let mut element = Element {
                emanation: self,
                overridden_accessors: Mutex::new(HashMap::new()),
                context: &context,
                cache: Mutex::new(HashMap::new()),
                unsaved_fields: Mutex::new(HashSet::new()),
            };
            domain.before_record(&mut element);
            f.execute(element);
        });
        let name = pretty_type_name::<F>();
        Kernel {
            domain,
            raw: device.compile_kernel_def_with_options(&kernel, options),
            context: Arc::new(context),
            debug_name: Some(name),
        }
    }
}

macro_rules! impl_kernel {
    () => {
        impl<D: Domain> Kernel<D, fn()> {
            pub fn dispatch_blocking(&self) {
                Domain::dispatch(self, ())
            }
            pub fn dispatch<'a>(&'a self) -> impl AddToComputeGraph<'a> {
                |graph: &mut ComputeGraph<'a>| Domain::dispatch_async(self, graph, ())
            }
        }
    };
    ($T0:ident: $S0:ident $(,$Tn:ident: $Sn:ident)*) => {
        impl<D: Domain, $T0: KernelArg + 'static $(, $Tn: KernelArg + 'static)*> Kernel<D, fn($T0 $(, $Tn)*)> {
            #[allow(non_snake_case)]
            #[allow(unused_variables)]
            pub fn dispatch_blocking<$S0: KernelArg $(, $Sn: KernelArg)*>(&self, $S0: $S0 $(, $Sn: $Sn)*)
            where ($S0, $($Sn),*): KernelArgs<S = fn($T0 $(, $Tn)*)> {
                Domain::dispatch(self, ($S0, $($Sn),*))
            }
            #[allow(non_snake_case)]
            #[allow(unused_variables)]
            pub fn dispatch<'a, $S0: KernelArg $(, $Sn: KernelArg)*>(&'a self, $S0: $S0 $(, $Sn: $Sn)*) -> impl AddToComputeGraph<'a>
            where ($S0, $($Sn),*): KernelArgs<S = fn($T0 $(, $Tn)*)> {
                |graph: &mut ComputeGraph<'a>| Domain::dispatch_async(self, graph, ($S0, $($Sn),*))
            }
        }
        impl_kernel!( $($Tn: $Sn),* );
    };
}

impl_kernel!(T0:S0, T1:S1, T2:S2, T3:S3, T4:S4, T5:S5, T6:S6, T7:S7, T8:S8, T9:S9, T10:S10, T11:S11, T12:S12, T13:S13, T14:S14);

pub trait KernelArgs {
    type S: KernelSignature;
    fn dispatch_with_size(
        kernel: &LuisaKernel<Self::S>,
        dispatch_size: [u32; 3],
        context: &Context,
        args: Self,
    );
    fn dispatch_with_size_async(
        kernel: &LuisaKernel<Self::S>,
        dispatch_size: [u32; 3],
        context: &Context,
        args: Self,
    ) -> Command<'static, 'static>;
}

macro_rules! impl_kernel_args {
    () => {
        impl KernelArgs for () {
            type S = fn();
            fn dispatch_with_size(
                kernel: &LuisaKernel<Self::S>,
                dispatch_size: [u32; 3],
                context: &Context,
                _args: Self,
            ) {
                kernel.dispatch(dispatch_size, context);
            }
            fn dispatch_with_size_async(
                kernel: &LuisaKernel<Self::S>,
                dispatch_size: [u32; 3],
                context: &Context,
                _args: Self,
            ) -> Command<'static, 'static> {
                kernel.dispatch_async(dispatch_size, context)
            }
        }
    };
    ($($Tn:ident: $n:tt),*) => {
        impl<$($Tn: KernelArg + AsKernelArg),*> KernelArgs for ($($Tn,)*) {
            type S = fn($(<$Tn as AsKernelArg>::Output),*);
            fn dispatch_with_size(
                kernel: &LuisaKernel<Self::S>,
                dispatch_size: [u32; 3],
                context: &Context,
                args: Self,
            ) {
                kernel.dispatch(dispatch_size, $(&args.$n,)* context);
            }
            fn dispatch_with_size_async(
                kernel: &LuisaKernel<Self::S>,
                dispatch_size: [u32; 3],
                context: &Context,
                args: Self,
            ) -> Command<'static, 'static> {
                kernel.dispatch_async(dispatch_size, $(&args.$n,)* context)
            }
        }
    }
}
impl_kernel_args!();
impl_kernel_args!(T0:0);
impl_kernel_args!(T0:0, T1:1);
impl_kernel_args!(T0:0, T1:1, T2:2);
impl_kernel_args!(T0:0, T1:1, T2:2, T3:3);
impl_kernel_args!(T0:0, T1:1, T2:2, T3:3, T4:4);
impl_kernel_args!(T0:0, T1:1, T2:2, T3:3, T4:4, T5:5);
impl_kernel_args!(T0:0, T1:1, T2:2, T3:3, T4:4, T5:5, T6:6);
impl_kernel_args!(T0:0, T1:1, T2:2, T3:3, T4:4, T5:5, T6:6, T7:7);
impl_kernel_args!(T0:0, T1:1, T2:2, T3:3, T4:4, T5:5, T6:6, T7:7, T8:8);
impl_kernel_args!(T0:0, T1:1, T2:2, T3:3, T4:4, T5:5, T6:6, T7:7, T8:8, T9:9);
impl_kernel_args!(T0:0, T1:1, T2:2, T3:3, T4:4, T5:5, T6:6, T7:7, T8:8, T9:9, T10:10);
impl_kernel_args!(T0:0, T1:1, T2:2, T3:3, T4:4, T5:5, T6:6, T7:7, T8:8, T9:9, T10:10, T11:11);
impl_kernel_args!(T0:0, T1:1, T2:2, T3:3, T4:4, T5:5, T6:6, T7:7, T8:8, T9:9, T10:10, T11:11, T12:12);
impl_kernel_args!(T0:0, T1:1, T2:2, T3:3, T4:4, T5:5, T6:6, T7:7, T8:8, T9:9, T10:10, T11:11, T12:12, T13:13);
impl_kernel_args!(T0:0, T1:1, T2:2, T3:3, T4:4, T5:5, T6:6, T7:7, T8:8, T9:9, T10:10, T11:11, T12:12, T13:13, T14:14);

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
