use super::*;
pub struct AccessCons<X: Access, L: AccessList>(PhantomData<fn() -> (X, L)>);
pub struct AccessNil;
pub trait AccessList {
    type Head;
    type Tail: AccessList;
}

impl AccessList for AccessNil {
    type Head = Paradox;
    type Tail = AccessNil;
}
impl<X: Access, L: AccessList> AccessList for AccessCons<X, L> {
    type Head = X;
    type Tail = L;
}

pub trait ListAccess {
    type List: AccessList;
    fn level() -> AccessLevel;
}

pub trait Access: ListAccess + 'static {
    type Downcast: ListAccess;
}

impl ListAccess for Paradox {
    type List = AccessNil;
    fn level() -> AccessLevel {
        AccessLevel(0)
    }
}
impl<X: Access> ListAccess for X {
    type List = AccessCons<X, <X::Downcast as ListAccess>::List>;
    fn level() -> AccessLevel {
        AccessLevel(X::Downcast::level().0 + 1)
    }
}

impl<V: Value> Access for Expr<V> {
    type Downcast = Paradox;
}
impl<V: Value> Access for Var<V> {
    type Downcast = Expr<V>;
}
impl<V: Value> Access for AtomicRef<V> {
    type Downcast = Var<V>;
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccessLevel(pub(crate) u8);
