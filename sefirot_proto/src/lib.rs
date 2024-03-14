use std::marker::PhantomData;

enum Paradox {}

enum LoopedAccess {}
enum NotLooped {}

struct HCons<H: Access, T>(PhantomData<fn() -> (H, T)>);
struct HNil;

trait ListAccess {
    type List;
}

impl ListAccess for Paradox {
    type List = HNil;
}

impl<X> ListAccess for X
where
    X: Access,
{
    type List = HCons<X, <X::Downcast as ListAccess>::List>;
}

trait Access: ListAccess {
    type Downcast: ListAccess;
}

struct Foo;
struct Bar;

impl Access for Bar {
    type Downcast = Foo;
}
impl Access for Foo {
    type Downcast = Paradox;
}

trait Mapping<X: Access>: AllowedMapping<X> {
    // fn as_chained(&self) -> &Self::Chain;
}

struct BasicMapping;
impl Mapping<Foo> for BasicMapping {}
impl Mapping<Bar> for BasicMapping {}

trait ListMapping<X> {}
impl<T, X: Access, Y> ListMapping<HCons<X, Y>> for T where T: Mapping<X> + ListMapping<Y> {}
impl<T> ListMapping<HNil> for T {}

trait AllowedMapping<X: Access>: ListMapping<<X as ListAccess>::List> {}
impl<T, X: Access> AllowedMapping<X> for T where T: ListMapping<<X as ListAccess>::List> {}

fn do_thing_with_mapping(m: impl AllowedMapping<Bar>) {}
fn main() {
    do_thing_with_mapping(BasicMapping);
}

// impl<X: Access, T: Mapping<X>> AllowedMapping<X> for T where T::Chain: AllowedMapping<X::Downcast> {}
