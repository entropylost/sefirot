use sefirot::ext_prelude::*;

use crate::GridDomain;

#[derive(Debug, Clone)]
pub struct MargolusDomain {
    grid: GridDomain,
}

impl Domain for MargolusDomain {
    type A = ();
    type I = Expr<Vec2<i32>>;
}
