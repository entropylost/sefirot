use std::rc::Rc;
use std::sync::Arc;
use std::vec;

use luisa::lang::types::vector::Vec2;
use parking_lot::{Mutex, RwLock};
use sefirot::ext_prelude::*;
use sefirot::field::FieldHandle;
use sefirot::mapping::buffer::StaticDomain;

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
            self.array
                .active_buffer
                .read(dispatch_id().x + self.array.max_active_tiles * self.index as u32),
            Context::new(kernel_context),
        )
    }
    fn dispatch(&self, _: (), args: KernelDispatch) -> NodeConfigs<'static> {
        args.dispatch([self.array.count_host.read()[self.index as usize], 1, 1])
    }
    fn contains_impl(&self, _: &Self::Index) -> Expr<bool> {
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
}

pub struct TileArrayParameters {
    pub device: Device,
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
    active_mask: AEField<u64, Vec2<u32>>,
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
            device,
            tile_size,
            array_size,
            max_active_tiles,
        } = parameters;
        let tile_domain = StaticDomain(array_size);
        let active_mask_buffer = device.create_buffer((array_size[0] * array_size[1]) as usize);
        let (active_mask, _active_mask_handle) = AEField::<u64, Vec2<u32>>::create_bind(
            "tile-array-active-mask",
            tile_domain.map_buffer_encoded(&encoder, active_mask_buffer.view(..)),
        );
        let active_buffer = device.create_buffer(64 * max_active_tiles as usize);
        let count_buffer: Buffer<u32> = device.create_buffer(64);
        let freelist = Mutex::new((0..64).collect());

        let calculate_buffers_kernel = Kernel::<fn()>::build(
            &device,
            &tile_domain,
            &track!(|el| {
                let active_mask = active_mask.expr(&el).var();
                while active_mask != 0 {
                    // TODO: This entire thing can be replaced with __clzll or __ffsll
                    let highest = active_mask & active_mask >> 1;
                    *active_mask ^= highest;
                    let level = highest.cast_f32().log2().cast_u32();
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
    pub fn update(&self) -> NodeConfigs<'static> {
        (
            self.count_buffer.copy_from_vec(vec![0; 64]),
            self.calculate_buffers_kernel.dispatch(),
            (
                self.count_buffer.copy_to_shared(&self.count_host),
                self.active_mask_buffer
                    .copy_from_vec(vec![0; self.active_mask_buffer.len()]),
            ),
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
