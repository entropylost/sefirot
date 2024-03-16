use std::any::TypeId;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::OnceLock;

use bevy::ecs::schedule::{NodeId, SystemTypeSet};
use bevy::prelude::*;
use bevy::utils::HashMap;
use luisa_compute::runtime::Device;
use sefirot::graph::{AsNodes, ComputeGraph, NodeHandle};
use sefirot::kernel::{Kernel, KernelSignature};
use sefirot::luisa as luisa_compute;

#[cfg(feature = "display")]
pub mod display;

pub mod luisa;

pub use bevy_sefirot_macro::kernel;

pub mod prelude {
    pub use bevy_sefirot_macro::kernel;
    pub use sefirot;
    pub use sefirot::prelude::*;

    pub use crate::luisa::{InitKernel, LuisaDevice as Device, LuisaPlugin};
}

pub struct KernelCell<S: KernelSignature, A: 'static = ()>(OnceLock<Kernel<S, A>>);

impl<S: KernelSignature, A: 'static> Deref for KernelCell<S, A> {
    type Target = Kernel<S, A>;
    fn deref(&self) -> &Self::Target {
        self.0.get().unwrap()
    }
}
impl<S: KernelSignature, A: 'static> KernelCell<S, A> {
    pub const fn default() -> Self {
        Self(OnceLock::new())
    }
    pub fn init(&self, kernel: Kernel<S, A>) {
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
    pub cached_graph: Option<ComputeGraph<'static>>,
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
            cached_graph: None,
            set_map: HashMap::new(),
            system_type_map: HashMap::new(),
            node_map: HashMap::new(),
        }
    }
    pub fn reinit(&mut self) {
        if let Some(graph) = &self.cached_graph {
            self.graph = graph.clone();
        } else {
            panic!("Cannot reinit an uninitialized graph.");
        }
    }

    /// Initialize the graph with the given schedule, using cached dependency and hierarchy graphs,
    /// as after the first schedule run, the systems are emptied.
    pub fn init_cached(&mut self, schedule: &Schedule) {
        if let Some(graph) = &self.cached_graph {
            self.graph = graph.clone();
        } else {
            self.init(schedule);
        }
    }

    // TODO: Just copy the graph from bevy over.
    // Also: Fix the add to graph thing (again) and get the particles example working.
    pub fn init(&mut self, schedule: &Schedule) {
        let graph = &mut self.graph;
        let set_map = &mut self.set_map;
        let system_type_map = &mut self.system_type_map;
        let node_map = &mut self.node_map;

        graph.clear();
        set_map.clear();
        system_type_map.clear();
        node_map.clear();

        let schedule = schedule.graph();

        let hierarchy = schedule.hierarchy().graph();
        let dependency = schedule.dependency().graph();

        for (node, set, _) in schedule.system_sets() {
            let handle = graph.add_single(format!("{:?}", set));
            set_map.insert(set.dyn_clone(), handle);
            node_map.insert(node, handle);
        }
        for (node, system, _) in schedule.systems() {
            let handle = graph.add_single(&*system.name());
            system_type_map
                .entry(system.type_id())
                .and_replace_entry_with(|_, _| Some(None))
                .or_insert(Some(handle));
            node_map.insert(node, handle);
        }

        for constraint in hierarchy.all_edges() {
            graph.add(node_map[&constraint.0].contains(node_map[&constraint.1]));
        }
        for constraint in dependency.all_edges() {
            graph.add(node_map[&constraint.0].before(node_map[&constraint.1]));
        }

        self.cached_graph = Some(graph.clone());
    }
    pub fn add_node<
        G: DerefMut<Target = MirrorGraph> + Resource + 'static,
        F: IntoSystem<I, N, M> + 'static,
        I: 'static,
        N: AsNodes<'static> + 'static,
        M: 'static,
    >(
        f: F,
    ) -> impl System<In = I, Out = ()> {
        f.pipe(AddNodeInterior::<G, F, I, N, M>(PhantomData))
    }
    pub fn execute_init(&mut self) {
        self.graph.execute_clear();
        self.reinit();
    }
    #[cfg(feature = "debug")]
    pub fn execute_init_dbg(&mut self) {
        self.graph.execute_clear_dbg();
        self.reinit();
    }
}

// This is implemented this way, instead of inline with a single function,
// as then it allows distinguishing between multiple systems as long as they are have any different chained parts.
#[allow(clippy::type_complexity)]
struct AddNodeInterior<G, F, I, N, M>(PhantomData<fn() -> (G, F, I, N, M)>);

struct AddNodeMarker;

impl<
        G: DerefMut<Target = MirrorGraph> + Resource + 'static,
        F: IntoSystem<I, N, M> + 'static,
        I: 'static,
        N: AsNodes<'static> + 'static,
        M: 'static,
    > SystemParamFunction<AddNodeMarker> for AddNodeInterior<G, F, I, N, M>
{
    type In = N;
    type Out = ();
    type Param = ResMut<'static, G>;

    fn run(&mut self, nodes: N, mut graph: ResMut<G>) {
        let nodes = graph.add_handles(nodes);
        let parent_set =
            graph.set_map[&(Box::new(system_type_set::<Self>()) as Box<dyn SystemSet>)];
        let children = graph
            .hierarchy()
            .edges(parent_set)
            .map(|(_, t, ())| t)
            .collect::<Vec<_>>();
        if children.len() != 1 {
            panic!(
                "Cannot add to graph with multiple systems within set {:?} ({:?}): Children are {:?}.",
                system_type_set::<F>(),
                parent_set,
                children
            );
        }
        let parent = children[0];
        graph.add(nodes.within(parent));
    }
}

fn system_type_set<F>() -> SystemTypeSet<F> {
    unsafe { std::mem::transmute::<(), SystemTypeSet<F>>(()) }
}

pub trait AsNodesStatic: AsNodes<'static> {}
impl<X> AsNodesStatic for X where X: AsNodes<'static> {}
