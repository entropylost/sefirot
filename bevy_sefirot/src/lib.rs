use bevy::ecs::schedule::NodeId;
use bevy::ecs::system::{CombinatorSystem, Pipe, SystemParamItem};
use bevy::utils::HashMap;
use bevy_luisa::luisa;

use luisa::runtime::Device;
use sefirot::domain::kernel::KernelSignature;
use sefirot::graph::{AsNode, ComputeGraph, NodeHandle};
use sefirot::prelude::{EmanationType, Kernel};
use std::any::TypeId;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::OnceLock;

pub use bevy_sefirot_macro::{add, init_kernel};

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

#[derive(DerefMut, Deref, Resource, Debug)]
pub struct MirrorGraph {
    #[deref]
    pub graph: ComputeGraph<'static>,
    pub set_map: HashMap<Box<dyn SystemSet>, NodeHandle>,
    pub system_type_map: HashMap<TypeId, Option<NodeHandle>>, // None if contradiction.
    pub node_map: HashMap<NodeId, NodeHandle>,
}

impl MirrorGraph {
    pub fn new(device: &Device, schedule: &Schedule) -> Self {
        let mut graph = Self::null(device);
        graph.init(schedule);
        graph
    }
    pub fn null(device: &Device) -> Self {
        Self {
            graph: ComputeGraph::new(device),
            set_map: HashMap::new(),
            system_type_map: HashMap::new(),
            node_map: HashMap::new(),
        }
    }
    pub fn init(&mut self, schedule: &Schedule) {
        let graph = &mut self.graph;
        let set_map = &mut self.set_map;
        let system_type_map = &mut self.system_type_map;
        let node_map = &mut self.node_map;

        graph.clear();
        set_map.clear();
        system_type_map.clear();
        node_map.clear();

        let hierarchy = schedule.graph().hierarchy().graph();
        let dependency = schedule.graph().dependency().graph();

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

struct AddToGraphSystemMarker;

struct AddToGraphSystem<G, F, I, M> {
    _marker: PhantomData<fn(G, F, I, M)>,
}
impl<
        G: DerefMut<Target = MirrorGraph> + Resource + 'static,
        F: IntoSystem<I, NodeHandle, M> + 'static,
        I: 'static,
        M: 'static,
    > SystemParamFunction<AddToGraphSystemMarker> for AddToGraphSystem<G, F, I, M>
{
    type In = NodeHandle;
    type Out = ();
    type Param = (ResMut<'static, G>,);
    fn run(&mut self, node: NodeHandle, (mut graph,): SystemParamItem<Self::Param>) {
        let parent = graph.system_type_map
            [&TypeId::of::<CombinatorSystem<Pipe, F::System, Self>>()]
            .expect("Cannot add to graph with multiple systems of the same type.");
        graph.graph.on(node).parent(parent);
    }
}

pub fn add_node<
    G: DerefMut<Target = MirrorGraph> + Resource + 'static,
    F: IntoSystem<I, NodeHandle, M> + 'static,
    I: 'static,
    M: 'static,
>(
    f: F,
) -> impl System<In = I, Out = ()> {
    f.pipe(AddToGraphSystem::<G, F, I, M> {
        _marker: PhantomData,
    })
}
