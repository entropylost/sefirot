use std::sync::{Arc, Weak};

use luisa::lang::ir::TypeOf;

use super::*;

impl<T: EmanationType> Emanation<T> {
    pub fn create_soa_fields<S: Structure>(
        &self,
        device: &Device,
        index: ArrayIndex<T>,
        prefix: Option<String>,
        values: &[S],
    ) -> S::Map<Field<Expr<__>, T>> {
        assert_eq!(values.len(), index.size as usize);
        S::apply(CreateArrayField {
            emanation: self,
            device,
            index,
            prefix,
            values,
        })
    }
    pub fn create_aos_fields<S: Structure>(
        &self,
        device: &Device,
        index: ArrayIndex<T>,
        prefix: Option<String>,
        values: &[S],
    ) -> S::Map<Field<Expr<__>, T>> {
        self.create_aos_fields_with_struct_field(device, index, prefix, values)
            .1
    }
    pub fn create_aos_fields_with_struct_field<S: Structure>(
        &self,
        device: &Device,
        index: ArrayIndex<T>,
        prefix: Option<String>,
        values: &[S],
    ) -> (Field<Expr<S>, T>, S::Map<Field<Expr<__>, T>>) {
        assert_eq!(values.len(), index.size as usize);
        let struct_field = self.create_field(None::<String>);
        let buffer = device.create_buffer_from_slice(values);
        let struct_accessor = BufferAccessor {
            index: index.clone(),
            buffer,
        };
        let struct_accessor = Arc::downgrade(&self.bind(struct_field, struct_accessor));

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
    device: &'a Device,
    index: ArrayIndex<T>,
    prefix: Option<String>,
    values: &'a [S],
}
impl<S: Structure, T: EmanationType> ValueMapping<S> for CreateArrayField<'_, S, T> {
    type M = Field<Expr<__>, T>;
    fn map<Z: Selector<S>>(&mut self, name: &'static str) -> Field<Expr<Z::Result>, T> {
        let field_name = self
            .prefix
            .as_ref()
            .map(|prefix| prefix.clone() + name)
            .unwrap_or(name.to_string());
        let buffer = self.device.create_buffer_from_fn(self.values.len(), |i| {
            Z::select_ref(&self.values[i]).clone()
        });
        self.emanation
            .create_array_field_from_buffer(self.index, Some(field_name), buffer)
    }
}

struct CreateStructArrayField<'a, S: Structure, T: EmanationType> {
    emanation: &'a Emanation<T>,
    prefix: Option<String>,
    struct_field: Field<Expr<S>, T>,
    struct_accessor: Weak<dyn DynAccessor<T>>,
}
impl<S: Structure, T: EmanationType> ValueMapping<S> for CreateStructArrayField<'_, S, T> {
    type M = Field<Expr<__>, T>;
    fn map<Z: Selector<S>>(&mut self, name: &'static str) -> Field<Expr<Z::Result>, T> {
        let field_name = self
            .prefix
            .as_ref()
            .map(|prefix| prefix.clone() + name)
            .unwrap_or(name.to_string());
        let field = self.emanation.create_field(Some(field_name));
        let accessor = StructArrayAccessor {
            struct_field: self.struct_field,
            struct_accessor: self.struct_accessor.clone(),
            _marker: PhantomData::<Z>,
        };
        self.emanation.bind(field, accessor);
        field
    }
}

pub trait Selector<S: Structure>: 'static {
    type Result: Value;
    fn select_expr(structure: &Expr<S>) -> Expr<Self::Result>;
    fn select_var(structure: &Var<S>) -> Var<Self::Result>;

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

impl<T: EmanationType> Mapping for Field<Expr<__>, T> {
    type Result<X: Value> = Field<Expr<X>, T>;
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
    struct_field: Field<Expr<S>, T>,
    struct_accessor: Weak<dyn DynAccessor<T>>,
    _marker: PhantomData<Z>,
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
            Z::select_var(&structure).store(value);
        } else {
            let _ = DynAccessor::get(&*struct_accessor, element, self.struct_field.raw);
            let mut cache = element.cache.lock();
            let structure = cache
                .get_mut(&self.struct_field.raw)
                .unwrap()
                .downcast_mut::<<BufferAccessor<S, T> as Accessor<T>>::C>()
                .unwrap();

            Z::select_var(&structure).store(value);
        }
        Ok(())
    }

    fn save(&self, _element: &Element<T>, _field: Field<Self::V, T>) {}

    fn can_write(&self) -> bool {
        true
    }
}
