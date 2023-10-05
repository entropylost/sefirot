use luisa_compute::prelude::*;
use luisa_compute::runtime::{KernelParameter, KernelSignature};

pub trait KernelFunction<S> {
    type Signature: KernelSignature;
    fn build(self, device: &Device, options: KernelBuildOptions) -> Kernel<Self::Signature>;
}

macro_rules! impl_kernel_function {
    () => {
        impl<F> KernelFunction<fn()> for F where F: Fn() {
            type Signature = fn();
            fn build(self, device: &Device, options: KernelBuildOptions) -> Kernel<Self::Signature> {
                Kernel::<fn()>::new_with_options(device, options, &self)
            }
        }
    };
    ($T0:ident $(,$Tn:ident)*) => {
        impl<F, $T0: KernelParameter $(,$Tn: KernelParameter)*> KernelFunction<fn($T0 $(,$Tn)*)> for F
        where
            F: Fn($T0 $(,$Tn)*),
        {
            type Signature = fn($T0::Arg $(,$Tn::Arg)*);
            fn build(self, device: &Device, options: KernelBuildOptions) -> Kernel<Self::Signature> {
                Kernel::<Self::Signature>::new_with_options(device, options, &self)
            }
        }
        impl_kernel_function!($($Tn),*);
    }
}

impl_kernel_function!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
