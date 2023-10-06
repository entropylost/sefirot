use std::sync::{Arc, Mutex};

use super::*;

pub struct ConstantAccessor<V: Value, T: EmanationType> {
    pub value: Arc<Mutex<V>>,
    _marker: PhantomData<T>,
}

impl<V: Value, T: EmanationType> Accessor<T> for ConstantAccessor<V, T> {
    type V = Expr<V>;
    type C = Expr<V>;

    fn get(
        &self,
        element: &mut Element<T>,
        field: Field<Self::V, T>,
    ) -> Result<Self::V, ReadError> {
        if let Some(expr) = self.get_cache(element, &field) {
            Ok(expr.clone())
        } else {
            let value = self.value.clone();
            let expr = element.context.bind(move || *value.lock().unwrap());
            self.insert_cache(element, field, expr.clone());
            Ok(expr)
        }
    }
    fn set(
        &self,
        _element: &mut Element<T>,
        _field: Field<Self::V, T>,
        _value: &Self::V,
    ) -> Result<(), WriteError> {
        Err(WriteError {
            message: "Cannot write to `ConstantAccessor`".to_string(),
        })
    }
    fn save(&self, _element: &mut Element<T>, _field: Field<Self::V, T>) {}

    fn can_write(&self) -> bool {
        false
    }
}
