use std::ops::Not;

use sefirot::ext_prelude::*;
use sefirot::field::access::{AccessCons, AccessList, ListAccess};
use sefirot::field::Access;
use sefirot::luisa::lang::types::vector::Vec2;
use sefirot::mapping::buffer::{HasPixelStorage, StaticDomain};
use sefirot::mapping::index::IndexMap;
use sefirot::mapping::ListMapping;

use crate::{Cell, GridDirection, GridDomain};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Facing {
    Horizontal,
    Vertical,
}
impl From<Facing> for u64 {
    fn from(facing: Facing) -> u64 {
        match facing {
            Facing::Horizontal => 0,
            Facing::Vertical => 1,
        }
    }
}
impl From<GridDirection> for Facing {
    fn from(dir: GridDirection) -> Facing {
        match dir {
            GridDirection::Up | GridDirection::Down => Facing::Horizontal,
            GridDirection::Left | GridDirection::Right => Facing::Vertical,
        }
    }
}
impl Facing {
    pub fn extract(&self, value: Expr<Vec2<f32>>) -> Expr<f32> {
        match self {
            Facing::Horizontal => value.y,
            Facing::Vertical => value.x,
        }
    }
    pub fn as_vec(&self) -> Vec2<i32> {
        match self {
            Facing::Horizontal => Vec2::new(0, 1),
            Facing::Vertical => Vec2::new(1, 0),
        }
    }
    pub fn as_vec_f32(&self) -> Vec2<f32> {
        let v = self.as_vec();
        Vec2::new(v.x as f32, v.y as f32)
    }
}

impl Not for Facing {
    type Output = Self;
    fn not(self) -> Self {
        match self {
            Facing::Horizontal => Facing::Vertical,
            Facing::Vertical => Facing::Horizontal,
        }
    }
}

#[derive(Copy, Clone)]
pub struct Edge {
    pos_cell: Cell,
    facing: Facing,
}

#[derive(Debug)]
pub struct DualGrid {
    horizontal: StaticDomain<2>,
    vertical: StaticDomain<2>,
    grid: GridDomain,
}
impl Clone for DualGrid {
    fn clone(&self) -> Self {
        Self {
            horizontal: self.horizontal,
            vertical: self.vertical,
            grid: self.grid.clone(),
        }
    }
}

// Note that the two mappings may have different sizes in non-wrapping grids.
#[derive(Debug, Clone, Copy)]
pub struct DualMapping<M> {
    pub horizontal: M,
    pub vertical: M,
}

impl<L: AccessList, X: Access + ListAccess<List = AccessCons<X, L>>, M> Mapping<X, Edge>
    for DualMapping<M>
where
    M: Mapping<X, Cell>,
    Self: ListMapping<L, Edge>,
{
    type Ext = ();
    fn access(&self, index: &Edge, ctx: &mut Context, binding: FieldBinding) -> X {
        match index.facing {
            Facing::Horizontal => {
                self.horizontal
                    .access(&index.pos_cell, ctx, binding.push(Facing::Horizontal))
            }
            Facing::Vertical => {
                self.vertical
                    .access(&index.pos_cell, ctx, binding.push(Facing::Vertical))
            }
        }
    }
    fn save(&self, ctx: &mut Context, binding: FieldBinding) {
        self.horizontal.save(ctx, binding.push(Facing::Horizontal));
        self.vertical.save(ctx, binding.push(Facing::Vertical));
    }
}

impl DualGrid {
    pub(crate) fn new(grid: GridDomain) -> Self {
        if grid.wrapping {
            Self {
                horizontal: grid.shifted_domain,
                vertical: grid.shifted_domain,
                grid,
            }
        } else {
            Self {
                horizontal: StaticDomain::<2>::new(grid.width(), grid.height() + 1),
                vertical: StaticDomain::<2>::new(grid.width() + 1, grid.height()),
                grid,
            }
        }
    }

    pub fn create_texture<V: HasPixelStorage>(&self, device: &Device) -> impl VMapping<V, Edge> {
        self.create_texture_with_storage(device, V::storage())
    }
    pub fn create_texture_with_storage<V: IoTexel>(
        &self,
        device: &Device,
        storage: PixelStorage,
    ) -> impl VMapping<V, Edge> {
        DualMapping {
            horizontal: IndexMap::new(
                self.grid.index,
                self.horizontal.create_tex2d_with_storage(device, storage),
            ),
            vertical: IndexMap::new(
                self.grid.index,
                self.vertical.create_tex2d_with_storage(device, storage),
            ),
        }
    }
    pub fn create_buffer<V: Value>(&self, device: &Device) -> impl AMapping<V, Edge> {
        if !self.grid.wrapping {
            panic!("Cannot create buffer for non-wrapping dual grid.");
        }
        DualMapping {
            horizontal: self.grid._create_buffer_typed(device),
            vertical: self.grid._create_buffer_typed(device),
        }
    }

    #[tracked]
    pub fn in_dir(&self, el: &Element<Cell>, dir: GridDirection) -> Element<Edge> {
        let facing = Facing::from(dir);
        let offset = match dir {
            GridDirection::Up => Vec2::new(0, 1),
            GridDirection::Right => Vec2::new(1, 0),
            _ => Vec2::splat(0),
        };
        let pos_cell = **el + offset;
        el.at(Edge { pos_cell, facing })
    }
}
