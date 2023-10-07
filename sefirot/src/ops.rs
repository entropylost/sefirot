use crate::emanation::FieldAccess;
use crate::prelude::*;
use luisa::prelude::tracked;

impl<V: Value, X, T: EmanationType> AddAssignExpr<X> for FieldAccess<'_, '_, Expr<V>, T>
where
    Var<V>: AddAssignExpr<X>,
{
    #[tracked]
    fn add_assign(mut self, other: X) {
        let v = (*self).var();
        *v += other;
        *self = **v;
    }
}
