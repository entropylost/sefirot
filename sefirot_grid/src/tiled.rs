use std::rc::Rc;
use std::sync::Arc;
use std::vec;

use luisa::lang::types::vector::Vec2;
use parking_lot::{Mutex, RwLock};
use sefirot::ext_prelude::*;
use sefirot::field::FieldHandle;
use sefirot::mapping::buffer::StaticDomain;
use sefirot::mapping::function::FnMapping;

use crate::encoder::{LinearEncoder, StaticDomainExt};

#[derive(Debug, Clone)]
pub struct TileDomain {
    array: Arc<TileArray>,
    index: u8,
}
impl Drop for TileDomain {
    fn drop(&mut self) {
        self.array.freelist.lock().push(self.index);
    }
}
impl DomainImpl for TileDomain {
    type Args = ();
    type Index = Expr<Vec2<u32>>;
    type Passthrough = ();
    #[tracked_nc]
    fn get_element(&self, kernel_context: Rc<KernelContext>, _: ()) -> Element<Self::Index> {
        Element::new(
            dispatch_id().xy()
                + self.array.tile_size
                    * self
                        .array
                        .active_buffer
                        .read(dispatch_id().z + self.array.max_active_tiles * self.index as u32),
            Context::new(kernel_context),
        )
    }
    fn dispatch(&self, _: (), args: KernelDispatch) -> NodeConfigs<'static> {
        if self.array.count_host.read()[self.index as usize] == 0 {
            ().into_node_configs()
        } else {
            args.dispatch([
                self.array.tile_size,
                self.array.tile_size,
                self.array.count_host.read()[self.index as usize],
            ])
        }
    }
    fn contains_impl(&self, _: &Element<Self::Index>) -> Expr<bool> {
        unimplemented!("Tile domain does not support contains.");
    }
}
impl TileDomain {
    #[tracked]
    pub fn activate(&self, el: &Element<Expr<Vec2<u32>>>) {
        self.array
            .active_mask
            .atomic(&el.at(**el / self.array.tile_size))
            .fetch_or(1_u64 << self.index);
    }
    #[tracked]
    pub fn active(&self) -> impl Mapping<Expr<bool>, Expr<Vec2<u32>>> {
        let tile_size = self.array.tile_size;
        let index = self.index;
        let active_mask = self.array.active_mask;
        FnMapping::new(move |idx, ctx| {
            let idx = idx / tile_size;
            (**active_mask).at_split(&idx, ctx) & (1_u64 << index) != 0
        })
    }
}

pub struct TileArrayParameters {
    pub tile_size: u32,
    pub array_size: [u32; 2],
    pub max_active_tiles: u32,
}

#[derive(Debug)]
pub struct TileArray {
    tile_size: u32,
    max_active_tiles: u32,
    _encoder: LinearEncoder,
    _tile_domain: StaticDomain<2>,
    pub edge: EEField<bool, Vec2<u32>>,
    _edge_handle: FieldHandle,
    // TODO: Expose active masks for each handle.
    pub active_mask: AEField<u64, Vec2<u32>>,
    _active_mask_handle: FieldHandle,
    active_mask_buffer: Buffer<u64>,
    active_buffer: Buffer<Vec2<u32>>,
    count_buffer: Buffer<u32>,
    count_host: Arc<RwLock<[u32; 64]>>,
    freelist: Mutex<Vec<u8>>,
    calculate_buffers_kernel: Kernel<fn()>,
}
impl TileArray {
    pub fn new(parameters: TileArrayParameters) -> Arc<Self> {
        debug_assert!(parameters.array_size[0] == parameters.array_size[1]);
        debug_assert!(parameters.array_size[0].is_power_of_two());
        let encoder = if parameters.array_size[0] <= 256 {
            LinearEncoder::morton_256()
        } else {
            LinearEncoder::morton()
        };
        Self::new_with_encoder(parameters, encoder)
    }
    pub fn new_with_encoder(parameters: TileArrayParameters, encoder: LinearEncoder) -> Arc<Self> {
        let TileArrayParameters {
            tile_size,
            array_size,
            max_active_tiles,
        } = parameters;
        let tile_domain = StaticDomain(array_size);

        // TODO: Add a separated-kernel invocation which does the edges separately.
        let (edge, _edge_handle) = EEField::<bool, Vec2<u32>>::create_bind(
            "tile-array-edge",
            FnMapping::<_, Expr<Vec2<u32>>, _>::new(track!(move |idx, _| {
                let idx = idx % tile_size;
                ((idx == 0) | (idx == tile_size - 1)).any()
            })),
        );

        let active_mask_buffer = device().create_buffer((array_size[0] * array_size[1]) as usize);
        let (active_mask, _active_mask_handle) = AEField::<u64, Vec2<u32>>::create_bind(
            "tile-array-active-mask",
            tile_domain.map_buffer_encoded(&encoder, active_mask_buffer.view(..)),
        );
        let active_buffer = device().create_buffer(64 * max_active_tiles as usize);
        let count_buffer: Buffer<u32> = device().create_buffer(64);
        let freelist = Mutex::new((0..64).collect());

        let calculate_buffers_kernel = Kernel::<fn()>::build(
            &tile_domain,
            &track!(|el| {
                let active_mask = active_mask.expr(&el).var();
                while active_mask != 0 {
                    let level = active_mask.trailing_zeros();
                    *active_mask ^= 1_u64 << level.cast_u64();
                    let index =
                        count_buffer.atomic_ref(level).fetch_add(1) + level * max_active_tiles;
                    active_buffer.write(index, *el);
                }
            }),
        );

        Arc::new(Self {
            tile_size,
            max_active_tiles,
            _encoder: encoder,
            _tile_domain: tile_domain,
            edge,
            _edge_handle,
            active_mask,
            active_mask_buffer,
            _active_mask_handle,
            active_buffer,
            count_buffer,
            count_host: Arc::new(RwLock::new([0; 64])),
            freelist,
            calculate_buffers_kernel,
        })
    }
    // TODO: Is this the most optimal way of doing it?
    // Probably not; letting the reset be done parallel would be better.
    pub fn reset(&self) -> NodeConfigs<'static> {
        (
            self.count_buffer.copy_from_vec(vec![0; 64]),
            self.active_mask_buffer
                .copy_from_vec(vec![0; self.active_mask_buffer.len()]),
        )
            .into_node_configs()
    }
    pub fn update(&self) -> NodeConfigs<'static> {
        (
            self.calculate_buffers_kernel.dispatch(),
            self.count_buffer.copy_to_shared(&self.count_host),
        )
            .chain()
    }
    pub fn allocate(self: &Arc<Self>) -> TileDomain {
        let index = self.freelist.lock().pop().unwrap();
        TileDomain {
            array: self.clone(),
            index,
        }
    }
}
