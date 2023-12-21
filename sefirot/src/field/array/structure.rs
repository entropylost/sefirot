use std::sync::{Arc, Weak};

use luisa::lang::ir::TypeOf;

use super::*;

pub trait IntoSlice<'a, V> {
    type R: Deref<Target = [V]>;
    fn into_slice(self, count: u32) -> Self::R;
}

impl<'a, V> IntoSlice<'a, V> for &'a [V] {
    type R = &'a [V];
    fn into_slice(self, count: u32) -> &'a [V] {
        assert_eq!(self.len(), count as usize);
        self
    }
}
impl<'a, V> IntoSlice<'a, V> for &'a Vec<V> {
    type R = &'a [V];
    fn into_slice(self, count: u32) -> &'a [V] {
        assert_eq!(self.len(), count as usize);
        self
    }
}
impl<'a, V, F> IntoSlice<'a, V> for F
where
    F: FnMut(u32) -> V,
{
    type R = Vec<V>;
    fn into_slice(self, count: u32) -> Vec<V> {
        (0..count).map(self).collect::<Vec<_>>()
    }
}

impl<T: EmanationType> Emanation<T> {
    // TODO: Change to use the `IntoBuffer`.
    pub fn create_soa_fields<'a, S: Structure>(
        &self,
        index: ArrayIndex<T>,
        prefix: &str,
        values: impl IntoSlice<'a, S>,
    ) -> S::Map<EField<__, T>> {
        let values = values.into_slice(index.size());
        S::apply(CreateArrayField {
            emanation: self,
            index,
            prefix: prefix.to_string(),
            values: &values,
        })
    }
    pub fn create_aos_fields<S: Structure>(
        &self,
        index: ArrayIndex<T>,
        prefix: &str,
        values: impl IntoBuffer<S>,
    ) -> S::Map<EField<__, T>> {
        self.create_aos_fields_with_struct_field(index, prefix, values)
            .1
    }
    pub fn create_aos_fields_with_struct_field<S: Structure>(
        &self,
        index: ArrayIndex<T>,
        prefix: &str,
        values: impl IntoBuffer<S>,
    ) -> (EField<S, T>, S::Map<EField<__, T>>) {
        let (buffer, handle) = values.into_buffer(&self.device, index.size());

        let prefix = prefix.to_string();
        let struct_field = *self.create_field(&(prefix.clone() + "struct"));
        let struct_accessor =
            Arc::downgrade(&self.on(struct_field).bind_accessor(BufferAccessor {
                index,
                buffer,
                handle,
                atomic: Mutex::new(None),
            }));

        let fields = S::apply(CreateStructArrayField {
            emanation: self,
            prefix,
            struct_field,
            struct_accessor,
        });
        (struct_field, fields)
    }
}

struct CreateArrayField<'a, S: Structure, T: EmanationType> {
    emanation: &'a Emanation<T>,
    index: ArrayIndex<T>,
    prefix: String,
    values: &'a [S],
}
impl<S: Structure, T: EmanationType> ValueMapping<S> for CreateArrayField<'_, S, T> {
    type M = EField<__, T>;
    fn map<Z: Selector<S>>(&mut self, name: &'static str) -> EField<Z::Result, T> {
        let field_name = self.prefix.clone() + name;
        let buffer = self
            .emanation
            .device
            .create_buffer_from_fn(self.values.len(), |i| *Z::select_ref(&self.values[i]));
        *self
            .emanation
            .create_field(&field_name)
            .bind_array(self.index, buffer)
    }
}

struct CreateStructArrayField<'a, S: Structure, T: EmanationType> {
    emanation: &'a Emanation<T>,
    prefix: String,
    struct_field: EField<S, T>,
    struct_accessor: Weak<dyn DynAccessor<T> + Send + Sync>,
}
impl<S: Structure, T: EmanationType> ValueMapping<S> for CreateStructArrayField<'_, S, T> {
    type M = EField<__, T>;
    fn map<Z: Selector<S>>(&mut self, name: &'static str) -> EField<Z::Result, T> {
        let field_name = self.prefix.clone() + name;
        *self
            .emanation
            .create_field(&field_name)
            .bind(StructArrayAccessor {
                struct_field: self.struct_field,
                struct_accessor: self.struct_accessor.clone(),
                _marker: PhantomData::<fn() -> Z>,
            })
    }
}

pub trait Selector<S: Structure>: 'static {
    type Result: Value;
    fn select_expr(structure: &Expr<S>) -> Expr<Self::Result>;
    fn select_var(structure: &Var<S>) -> Var<Self::Result>;
    fn select_atomic(structure: &AtomicRef<S>) -> AtomicRef<Self::Result>;

    fn select_ref(structure: &S) -> &Self::Result;
    fn select_mut(structure: &mut S) -> &mut Self::Result;
    fn select(structure: S) -> Self::Result;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum __ {}
const _: () = {
    impl TypeOf for __ {
        fn type_() -> luisa::lang::ir::CArc<luisa::lang::ir::Type> {
            panic!("__ is a dummy type for creating `Mapping`s and should not be used");
        }
    }
    luisa::impl_simple_expr_proxy!([] __Expr for __);
    luisa::impl_simple_var_proxy!([] __Var for __);
    luisa::impl_simple_atomic_ref_proxy!([] __Ref for __);
    impl Value for __ {
        type Expr = __Expr;
        type Var = __Var;
        type AtomicRef = __Ref;
    }
};

pub trait Mapping: 'static {
    type Result<X: Value>;
}

impl Mapping for Buffer<__> {
    type Result<X: Value> = Buffer<X>;
}

impl Mapping for Expr<__> {
    type Result<X: Value> = Expr<X>;
}

impl<A: Mapping, T: EmanationType> Mapping for Field<A, T> {
    type Result<X: Value> = Field<A::Result<X>, T>;
}

pub trait ValueMapping<S: Structure> {
    type M: Mapping;
    fn map<Z: Selector<S>>(
        &mut self,
        name: &'static str,
    ) -> <Self::M as Mapping>::Result<Z::Result>;
}

pub trait Structure: Value {
    type Map<M: Mapping>;
    fn apply<M: Mapping>(f: impl ValueMapping<Self, M = M>) -> Self::Map<M>;
}

struct StructArrayAccessor<Z: Selector<S>, S: Structure, T: EmanationType> {
    struct_field: EField<S, T>,
    struct_accessor: Weak<dyn DynAccessor<T> + Send + Sync>,
    _marker: PhantomData<fn() -> Z>,
}

impl<Z: Selector<S>, S: Structure, T: EmanationType> Accessor<T> for StructArrayAccessor<Z, S, T> {
    type V = Expr<Z::Result>;
    type C = ();

    fn get(&self, element: &Element<T>, _field: Field<Self::V, T>) -> Result<Self::V, ReadError> {
        let structure = element.get(self.struct_field)?;
        Ok(Z::select_expr(&structure))
    }

    fn set(
        &self,
        element: &Element<T>,
        _field: Field<Self::V, T>,
        value: &Self::V,
    ) -> Result<(), WriteError> {
        let struct_accessor = self.struct_accessor.upgrade().unwrap();
        element.unsaved_fields.lock().insert(self.struct_field.raw);
        if let Some(structure) = element.cache.lock().get_mut(&self.struct_field.raw) {
            let structure = structure
                .downcast_mut::<<BufferAccessor<S, T> as Accessor<T>>::C>()
                .unwrap();
            Z::select_var(structure).store(value);
        } else {
            let _ = DynAccessor::get(&*struct_accessor, element, self.struct_field.raw);
            let mut cache = element.cache.lock();
            let structure = cache
                .get_mut(&self.struct_field.raw)
                .unwrap()
                .downcast_mut::<<BufferAccessor<S, T> as Accessor<T>>::C>()
                .unwrap();

            Z::select_var(structure).store(value);
        }
        Ok(())
    }

    fn save(&self, _element: &Element<T>, _field: Field<Self::V, T>) {}

    fn can_write(&self) -> bool {
        true
    }

    fn get_atomic(&self, emanation: &Emanation<T>) -> Option<RawFieldHandle> {
        Some(
            emanation
                .on(self.struct_field)
                .atomic()
                .map(|x, _| Z::select_atomic(&x))
                .raw(),
        )
    }
}
