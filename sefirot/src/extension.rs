use std::any::Any;
use std::marker::PhantomData;

use crate::ext_prelude::*;
use crate::field::{FieldIndex, FIELDS};
use crate::mapping::DynMapping;

pub fn find_extension<T: Send + Sync + 'static>(
    extensions: &[Box<dyn Any + Send + Sync>],
) -> Option<&T> {
    extensions.iter().find_map(|ext| ext.downcast_ref::<T>())
}

pub trait Extension<M: ?Sized>: Send + Sync + 'static {
    fn load(m: &M) -> Self;
}

pub trait ExtensionList<M: ?Sized> {
    type Head: Extension<M>;
    type Tail: ExtensionList<M>;
    fn load_all(m: &M) -> Vec<Box<dyn Any + Send + Sync>> {
        let mut extensions = Self::Tail::load_all(m);
        extensions.push(Box::new(Self::Head::load(m)));
        extensions
    }
}

impl<M: ?Sized> Extension<M> for () {
    fn load(_m: &M) -> Self {}
}

impl<M: ?Sized> ExtensionList<M> for () {
    type Head = ();
    type Tail = ();
    fn load_all(_m: &M) -> Vec<Box<dyn Any + Send + Sync>> {
        vec![]
    }
}

macro_rules! impl_el_tuples {
    () => {};
    ($S0:ident $(,$Sn:ident)*) => {
        impl<M: ?Sized, $S0: Extension<M> $(,$Sn: Extension<M>)*> ExtensionList<M> for ($S0, $($Sn,)*) {
            type Head = $S0;
            type Tail = ($($Sn,)*);
        }
        impl_el_tuples!($($Sn),*);
    };
}
impl_el_tuples!(A, B, C, D, E, F, G, H, I, J, K, L);

pub struct CopyExt(Box<dyn CopyAny + Send + Sync>);

impl<M> Extension<M> for CopyExt
where
    M: CopyImpl + Send + Sync,
{
    fn load(_m: &M) -> Self {
        CopyExt(Box::new(CopyExtInner::<M>(PhantomData)))
    }
}

trait CopyAny: 'static {
    fn copy_to(&self, this: &dyn Any, dst: &dyn DynMapping) -> Option<NodeConfigs<'static>>;
    fn copy_from(&self, this: &dyn Any, src: &dyn DynMapping) -> Option<NodeConfigs<'static>>;
}
struct CopyExtInner<M: CopyImpl>(PhantomData<M>);
impl<M: CopyImpl> CopyAny for CopyExtInner<M> {
    fn copy_to(&self, this: &dyn Any, dst: &dyn DynMapping) -> Option<NodeConfigs<'static>> {
        this.downcast_ref::<M>().unwrap().copy_to(dst)
    }
    fn copy_from(&self, this: &dyn Any, src: &dyn DynMapping) -> Option<NodeConfigs<'static>> {
        this.downcast_ref::<M>().unwrap().copy_from(src)
    }
}

impl CopyImpl for dyn DynMapping {
    fn copy_to(&self, dst: &dyn DynMapping) -> Option<NodeConfigs<'static>> {
        if let Some(CopyExt(copy)) = find_extension::<CopyExt>(self.extensions()) {
            copy.copy_to(self.as_any(), dst)
        } else {
            None
        }
    }
    fn copy_from(&self, src: &dyn DynMapping) -> Option<NodeConfigs<'static>> {
        if let Some(CopyExt(copy)) = find_extension::<CopyExt>(self.extensions()) {
            copy.copy_from(self.as_any(), src)
        } else {
            None
        }
    }
}

pub trait CopyImpl: 'static {
    fn copy_to(&self, dst: &dyn DynMapping) -> Option<NodeConfigs<'static>>;
    fn copy_from(&self, src: &dyn DynMapping) -> Option<NodeConfigs<'static>>;
}

pub trait CopyExtension<T> {
    fn copy(&self, dst: &T) -> NodeConfigs<'static> {
        self.copy_opt(dst).unwrap()
    }
    fn copy_opt(&self, dst: &T) -> Option<NodeConfigs<'static>>;
}
impl<T: Value, I: FieldIndex> CopyExtension<VField<T, I>> for EField<T, I> {
    fn copy_opt(&self, dst: &VField<T, I>) -> Option<NodeConfigs<'static>> {
        let src = FIELDS.get(&self.id).unwrap();
        let dst = FIELDS.get(&dst.id).unwrap();
        let src: &dyn DynMapping = &**src.binding.as_ref()?;
        let dst: &dyn DynMapping = &**dst.binding.as_ref()?;
        src.copy_to(dst).or_else(|| dst.copy_from(src))
    }
}
