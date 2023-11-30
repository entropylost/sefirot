use bevy::ecs::schedule::NodeId;
use bevy::utils::HashMap;
use bevy_luisa::luisa;

use luisa::runtime::Device;
use sefirot::domain::kernel::KernelSignature;
use sefirot::graph::{AsNode, ComputeGraph, NodeHandle};
use sefirot::prelude::{EmanationType, Kernel};
use std::any::TypeId;
use std::ops::Deref;
use std::sync::OnceLock;

pub use bevy_sefirot_macro::init_kernel;

use bevy::prelude::*;

pub mod prelude {
    pub use bevy_luisa::{LuisaDevice, LuisaPlugin};
    pub use bevy_sefirot_macro::init_kernel;
    pub use sefirot::prelude::*;
    pub use {bevy_luisa, sefirot};
}

pub struct KernelCell<T: EmanationType, S: KernelSignature, A = ()>(OnceLock<Kernel<T, S, A>>);

impl<T: EmanationType, S: KernelSignature, A> Deref for KernelCell<T, S, A> {
    type Target = Kernel<T, S, A>;
    fn deref(&self) -> &Self::Target {
        self.0.get().unwrap()
    }
}
impl<T: EmanationType, S: KernelSignature, A> KernelCell<T, S, A> {
    pub const fn default() -> Self {
        Self(OnceLock::new())
    }
    pub fn init(&self, kernel: Kernel<T, S, A>) {
        self.0.set(kernel).ok().unwrap();
    }
}

// Compute Graph Builder Design:
// Function: Create compute graph from Schedule (run every schedule run).
// Also returns a mapping between NodeId and NodeHandle.
// Then, add a `#[add]` macro which makes a system outputting `impl AsNode`
// to add the output to  the graph, using the mapping.
// Also need some way of actually getting a `NodeHandle` from a `impl SystemSet`.
// Can use the SystemSet hash, and just reuse the SystemSet wrapper for each System that hopefully exists (test this)?

#[derive(DerefMut, Deref, Resource)]
pub struct MirrorGraph {
    #[deref]
    pub graph: ComputeGraph<'static>,
    pub set_map: HashMap<Box<dyn SystemSet>, NodeHandle>,
    pub system_type_map: HashMap<TypeId, Option<NodeHandle>>, // None if contradiction.
    pub node_map: HashMap<NodeId, NodeHandle>,
}

impl MirrorGraph {
    pub fn new(device: &Device, schedule: &Schedule) -> Self {
        let mut graph = ComputeGraph::new(device);

        let hierarchy = schedule.graph().hierarchy().graph();
        let dependency = schedule.graph().dependency().graph();

        let mut set_map = HashMap::new();
        let mut system_type_map = HashMap::new();
        let mut node_map = HashMap::new();

        for (node, set, _) in schedule.graph().system_sets() {
            let handle = *graph.container();
            set_map.insert(set.dyn_clone(), handle);
            node_map.insert(node, handle);
        }
        for (node, system, _) in schedule.graph().systems() {
            let handle = *graph.container();
            system_type_map
                .entry(system.type_id())
                .and_replace_entry_with(|_, _| Some(None))
                .or_insert(Some(handle));
            node_map.insert(node, handle);
        }

        for (node, handle) in node_map.iter() {
            graph
                .on(*handle)
                .children(hierarchy.edges(*node).map(|e| &node_map[&e.1]))
                .before_all(dependency.edges(*node).map(|e| &node_map[&e.1]));
        }

        MirrorGraph {
            graph,
            set_map,
            system_type_map,
            node_map,
        }
    }
    pub fn add_to_system<F: IntoSystem<(), (), M> + 'static, M>(
        &mut self,
        _f: F,
        node: impl AsNode<'static>,
    ) -> NodeHandle {
        *self.graph.add(node).parent(
            self.system_type_map[&TypeId::of::<F>()]
                .expect("Cannot add to graph with multiple systems of the same type."),
        )
    }
}
