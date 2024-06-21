use std::rc::Rc;

use luisa::lang::types::vector::Vec2;
use sefirot::ext_prelude::*;
use sefirot::mapping::function::FnMapping;

use crate::{Cell, GridDomain};

#[derive(Debug, Clone)]
pub struct CheckerboardPattern {
    pub(crate) grid: GridDomain,
}

impl DomainImpl for CheckerboardPattern {
    type Args = ();
    type Index = Cell;
    type Passthrough = bool;
    #[tracked_nc]
    fn get_element(
        &self,
        kernel_context: Rc<KernelContext>,
        parity: Expr<bool>,
    ) -> Element<Self::Index> {
        let uindex = Vec2::expr(
            dispatch_id().x,
            dispatch_id().y * 2 + (dispatch_id().x + parity.cast::<u32>()) % 2,
        );
        let mut context = Context::new(kernel_context);
        let index = uindex.cast_i32() + self.grid.offset_field.at_split(&(), &mut context);
        context.bind_local(self.grid.index, FnMapping::new(move |_el, _ctx| uindex));
        Element::new(index, context)
    }
    fn dispatch(
        &self,
        _: Self::Args,
        args: KernelDispatch<Self::Passthrough>,
    ) -> NodeConfigs<'static> {
        let [w, h] = self.grid.size();
        let prefix = args
            .kernel_name()
            .map_or_else(String::new, |name| format!("{}-", name));
        (
            args.dispatch_with([w, h / 2, 1], false)
                .debug(format!("{}alpha", prefix)),
            args.dispatch_with([w, h / 2, 1], true)
                .debug(format!("{}beta", prefix)),
        )
            .chain()
    }
    fn contains_impl(&self, el: &Element<Self::Index>) -> Expr<bool> {
        self.grid.contains(el)
    }
}

/// A pattern that invokes the function on 2x2 blocks separately.
/// See the [Margolus neighborhood](https://en.wikipedia.org/wiki/Block_cellular_automaton).
#[derive(Debug, Clone)]
pub struct MargolusPattern {
    pub(crate) grid: GridDomain,
}

impl DomainImpl for MargolusPattern {
    type Args = ();
    type Index = Cell;
    type Passthrough = Vec2<bool>;
    #[tracked_nc]
    fn get_element(
        &self,
        kernel_context: Rc<KernelContext>,
        offset: Expr<Vec2<bool>>,
    ) -> Element<Self::Index> {
        let uindex = dispatch_id().xy() * 2 + offset.cast::<u32>();
        let mut context = Context::new(kernel_context);
        let index = uindex.cast_i32() + self.grid.offset_field.at_split(&(), &mut context);
        context.bind_local(self.grid.index, FnMapping::new(move |_el, _ctx| uindex));
        Element::new(index, context)
    }
    fn dispatch(
        &self,
        _: Self::Args,
        args: KernelDispatch<Self::Passthrough>,
    ) -> NodeConfigs<'static> {
        let prefix = args
            .kernel_name()
            .map_or_else(String::new, |name| format!("{}-", name));
        let [w, h] = self.grid.size();
        if self.grid.wrapping {
            let size = [w / 2, h / 2, 1];
            (
                args.dispatch_with(size, Vec2::new(false, false))
                    .debug(format!("{}00", prefix)),
                args.dispatch_with(size, Vec2::new(false, true))
                    .debug(format!("{}01", prefix)),
                args.dispatch_with(size, Vec2::new(true, true))
                    .debug(format!("{}11", prefix)),
                args.dispatch_with(size, Vec2::new(true, false))
                    .debug(format!("{}10", prefix)),
            )
                .chain()
        } else {
            (
                args.dispatch_with([w / 2, h / 2, 1], Vec2::new(false, false))
                    .debug(format!("{}00", prefix)),
                args.dispatch_with([w / 2, (h - 1) / 2, 1], Vec2::new(false, true))
                    .debug(format!("{}01", prefix)),
                args.dispatch_with([(w - 1) / 2, (h - 1) / 2, 1], Vec2::new(true, true))
                    .debug(format!("{}11", prefix)),
                args.dispatch_with([(w - 1) / 2, h / 2, 1], Vec2::new(true, false))
                    .debug(format!("{}10", prefix)),
            )
                .chain()
        }
    }
    fn contains_impl(&self, index: &Element<Self::Index>) -> Expr<bool> {
        self.grid.contains(index)
    }
}
impl MargolusPattern {
    pub fn indexed(self) -> MargolusIndexedPattern {
        MargolusIndexedPattern { grid: self.grid }
    }
}

#[derive(Debug, Clone)]
pub struct MargolusIndexedPattern {
    pub(crate) grid: GridDomain,
}

impl DomainImpl for MargolusIndexedPattern {
    type Args = u8;
    type Index = Cell;
    type Passthrough = Vec2<bool>;
    #[tracked_nc]
    fn get_element(
        &self,
        kernel_context: Rc<KernelContext>,
        offset: Expr<Vec2<bool>>,
    ) -> Element<Self::Index> {
        let uindex = dispatch_id().xy() * 2 + offset.cast::<u32>();
        let mut context = Context::new(kernel_context);
        let index = uindex.cast_i32() + self.grid.offset_field.at_split(&(), &mut context);
        context.bind_local(self.grid.index, FnMapping::new(move |_el, _ctx| uindex));
        Element::new(index, context)
    }
    fn dispatch(&self, index: u8, args: KernelDispatch<Self::Passthrough>) -> NodeConfigs<'static> {
        let prefix = args
            .kernel_name()
            .map_or_else(String::new, |name| format!("{}-", name));
        let [w, h] = self.grid.size();
        let index = index % 4;
        let offset = [
            Vec2::new(false, false),
            Vec2::new(false, true),
            Vec2::new(true, true),
            Vec2::new(true, false),
        ][index as usize];
        let name = format!("{}{}{}", prefix, offset.x as u8, offset.y as u8);
        let size = if self.grid.wrapping {
            [w / 2, h / 2, 1]
        } else {
            [
                if offset.x { (w - 1) / 2 } else { w / 2 },
                if offset.y { (h - 1) / 2 } else { h / 2 },
                1,
            ]
        };
        args.dispatch_with(size, offset).debug(name)
    }
    fn contains_impl(&self, index: &Element<Self::Index>) -> Expr<bool> {
        self.grid.contains(index)
    }
}
