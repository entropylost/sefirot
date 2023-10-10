use luisa::prelude::tracked;

use crate::field::FieldAccess;
use crate::prelude::*;

macro_rules! impl_assignop {
    ($Trait:ident: $fn:tt; $op:tt) => {
        impl<V: Value, X, T: EmanationType> $Trait<X> for FieldAccess<'_, Expr<V>, T>
        where
            Var<V>: $Trait<X>,
        {
            #[tracked]
            fn $fn(mut self, other: X) {
                let v = (*self).var();
                *v $op other;
                *self = **v;
            }
        }
    }
}
impl_assignop!(AddAssignExpr: add_assign; +=);
impl_assignop!(SubAssignExpr: sub_assign; -=);
impl_assignop!(MulAssignExpr: mul_assign; *=);
impl_assignop!(DivAssignExpr: div_assign; /=);
impl_assignop!(RemAssignExpr: rem_assign; %=);
impl_assignop!(BitAndAssignExpr: bitand_assign; &=);
impl_assignop!(BitOrAssignExpr: bitor_assign; |=);
impl_assignop!(BitXorAssignExpr: bitxor_assign; ^=);
impl_assignop!(ShlAssignExpr: shl_assign; <<=);
impl_assignop!(ShrAssignExpr: shr_assign; >>=);
