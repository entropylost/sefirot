use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Exclusive;

use petgraph::algo::toposort;
use petgraph::dot::Dot;
use petgraph::graphmap::DiGraphMap;
use petgraph::Direction;
use static_assertions::assert_impl_all;

use self::tag::{DynTag, Tag, TagMap};
use crate::prelude::*;
use crate::utils::FnRelease;

pub mod copy;
pub mod tag;
pub use copy::CopyExt;

pub fn dot_graph(compute: &ComputeGraph<'_>, graph: &DiGraphMap<NodeHandle, ()>) -> String {
    struct StrDbg(String);
    impl Debug for StrDbg {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    struct NoDbg;
    impl Debug for NoDbg {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            Ok(())
        }
    }

    let graph = graph.clone().into_graph::<u32>().map(
        |_, n| {
            let mut s = compute.name_of(*n).to_string();
            s.truncate(30);
            StrDbg(s)
        },
        |_, _| NoDbg,
    );

    format!("{:?}", Dot::new(&graph))
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NodeHandle {
    Container(usize),
    Command(usize),
}

pub struct CommandNode<'a> {
    pub command: Exclusive<Command<'a, 'a>>,
    pub debug_name: String,
}
impl Debug for CommandNode<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CommandNode {{ debug_name: {:?}, .. }}", self.debug_name)
    }
}

#[derive(Debug, Clone)]
pub struct ContainerNode {
    pub debug_name: String,
}

#[cfg_attr(
    feature = "bevy",
    derive(bevy_ecs::prelude::Resource, bevy_ecs::prelude::Component)
)]
pub struct ComputeGraph<'a> {
    tags: TagMap<NodeHandle>,
    commands: Vec<CommandNode<'a>>,
    containers: Vec<ContainerNode>,
    hierarchy: DiGraphMap<NodeHandle, ()>,
    dependency: DiGraphMap<NodeHandle, ()>,
    device: Device,
    // Resources to be released after the graph is executed.
    release: Vec<Exclusive<Box<dyn Send>>>,
}
assert_impl_all!(ComputeGraph: Send, Sync);
impl Debug for ComputeGraph<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComputeGraph")
            .field("commands", &self.commands)
            .field("containers", &self.containers)
            .field("hierarchy", &self.hierarchy)
            .field("dependency", &self.dependency)
            .finish()
    }
}
impl Clone for ComputeGraph<'_> {
    fn clone(&self) -> Self {
        assert!(self.commands.is_empty());
        assert!(self.release.is_empty());
        Self {
            tags: self.tags.clone(),
            commands: Vec::new(),
            containers: self.containers.clone(),
            hierarchy: self.hierarchy.clone(),
            dependency: self.dependency.clone(),
            device: self.device.clone(),
            release: Vec::new(),
        }
    }
}
impl<'a> ComputeGraph<'a> {
    pub fn new(device: &Device) -> Self {
        Self {
            tags: TagMap::new(),
            commands: Vec::new(),
            containers: Vec::new(),
            hierarchy: DiGraphMap::new(),
            dependency: DiGraphMap::new(),
            device: device.clone(),
            release: Vec::new(),
        }
    }

    pub fn set_dependency(&mut self, dependency: DiGraphMap<NodeHandle, ()>) {
        self.dependency = dependency;
    }
    pub fn set_hierarchy(&mut self, hierarchy: DiGraphMap<NodeHandle, ()>) {
        self.hierarchy = hierarchy;
    }

    pub fn dependency(&self) -> &DiGraphMap<NodeHandle, ()> {
        &self.dependency
    }
    pub fn hierarchy(&self) -> &DiGraphMap<NodeHandle, ()> {
        &self.hierarchy
    }
    pub fn device(&self) -> &Device {
        &self.device
    }

    fn order(&self) -> Vec<NodeHandle> {
        let mut dependency = self.dependency.clone();
        let mut hierarchy = self.hierarchy.clone();

        let mut next_fence = self.containers.len();

        for id in 0..self.containers.len() {
            let node = NodeHandle::Container(id);
            let before = NodeHandle::Container(next_fence);
            next_fence += 1;
            let after = NodeHandle::Container(next_fence);
            next_fence += 1;

            dependency.add_edge(before, after, ());

            for child in hierarchy
                .neighbors_directed(node, Direction::Outgoing)
                .collect::<Vec<_>>()
            {
                dependency.add_edge(before, child, ());
                dependency.add_edge(child, after, ());
            }
            for parent in hierarchy
                .neighbors_directed(node, Direction::Incoming)
                .collect::<Vec<_>>()
            {
                hierarchy.add_edge(parent, before, ());
                hierarchy.add_edge(parent, after, ());
            }

            for next in dependency
                .neighbors_directed(node, Direction::Outgoing)
                .collect::<Vec<_>>()
            {
                dependency.add_edge(after, next, ());
            }
            for prev in dependency
                .neighbors_directed(node, Direction::Incoming)
                .collect::<Vec<_>>()
            {
                dependency.add_edge(prev, before, ());
            }

            dependency.remove_node(node);
            hierarchy.remove_node(node);
        }

        for command in 0..self.commands.len() {
            dependency.add_node(NodeHandle::Command(command));
        }

        toposort(&dependency, None)
            .expect("Compute graph is cyclic.")
            .into_iter()
            .filter(|node| matches!(node, NodeHandle::Command(_)))
            .collect()
    }

    fn commands_order(&mut self) -> Vec<CommandNode<'a>> {
        let order = self.order();
        let mut commands = std::mem::take(&mut self.commands)
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>();
        order
            .into_iter()
            .map(|handle| {
                let NodeHandle::Command(idx) = handle else {
                    panic!("Order should only produce command nodes.");
                };
                commands[idx]
                    .take()
                    .expect("Cannot have duplicate commands.")
            })
            .collect::<Vec<_>>()
    }

    // TODO: This currently does not parallelize anything.
    /// Consumes the graph, executing it.
    pub fn execute(&mut self) {
        let mut this = std::mem::replace(self, Self::new(&self.device));

        let commands = this.commands_order();
        let scope = this.device.default_stream().scope();
        scope.submit_with_callback(commands.into_iter().map(|c| c.command.into_inner()), || {
            drop(this.release);
        });
    }

    /// Executes the graph without parallelism, printing debug information.
    #[cfg(feature = "debug")]
    pub fn execute_dbg(&mut self) {
        use tracing::info;

        let mut this = std::mem::replace(self, Self::new(&self.device));

        let commands = this.commands_order();
        for command in commands {
            info!("Executing {:?}", command.debug_name);
            let scope = this.device.default_stream().scope();
            scope.submit(std::iter::once(command.command.into_inner()));
        }
    }

    #[cfg(feature = "trace")]
    pub fn execute_timings(&mut self) -> Vec<(String, f32)> {
        use std::ptr;

        use cuda_device_sys::*;

        let mut this = std::mem::replace(self, Self::new(&self.device));

        let mut timings = vec![];

        let stream = this.device.default_stream().native_handle() as CUstream;

        let mut context: CUcontext =
            unsafe { std::mem::transmute::<*mut _, CUcontext>(this.device.native_handle()) };

        unsafe {
            assert_eq!(cuCtxPushCurrent_v2(context), 0);
        }

        let mut start: CUevent = ptr::null_mut();
        let mut end: CUevent = ptr::null_mut();

        unsafe {
            assert_eq!(cuEventCreate(&mut start as *mut CUevent, 0), 0);
            assert_eq!(cuEventCreate(&mut end as *mut CUevent, 0), 0);
        }

        let commands = this.commands_order();
        for command in commands {
            unsafe {
                assert_eq!(cuEventRecord(start, stream), 0);
            }

            let scope = this.device.default_stream().scope();
            scope.submit(std::iter::once(command.command.into_inner()));

            let mut elapsed_time: f32 = 0.0;

            unsafe {
                assert_eq!(cuEventRecord(end, stream), 0);
                assert_eq!(cuEventSynchronize(end), 0);
                assert_eq!(
                    cuEventElapsedTime(&mut elapsed_time as *mut f32, start, end),
                    0
                );
            }
            timings.push((command.debug_name, elapsed_time));
        }

        unsafe {
            assert_eq!(cuEventDestroy_v2(start), 0);
            assert_eq!(cuEventDestroy_v2(end), 0);
            assert_eq!(cuCtxPopCurrent_v2(&mut context as *mut CUcontext), 0);
        }

        timings
    }
    #[cfg(feature = "trace")]
    pub fn execute_trace(&mut self) {
        let timings = self.execute_timings();

        let _guard = tracing::info_span!("graph timings").entered();

        for (name, time) in timings {
            let start = std::time::Instant::now();
            let _guard = tracing::info_span!("command", name).entered();
            while start.elapsed().as_millis_f32() < time {
                std::hint::spin_loop();
            }
            let dt = start.elapsed().as_millis_f32();
            if dt > time + 0.01 {
                let message = format!(
                    "command {:?} took {}ms while being profiled as {}ms",
                    name, time, dt
                );
                tracing::warn!(name:"overrun", message);
            }
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new(&self.device);
    }

    pub fn handle(&self, tag: impl Tag) -> Option<NodeHandle> {
        self.tags.get(tag).copied()
    }

    fn set_debug(&mut self, handle: NodeHandle, name: String) {
        match handle {
            NodeHandle::Container(idx) => {
                self.containers[idx].debug_name = name;
            }
            NodeHandle::Command(idx) => {
                self.commands[idx].debug_name = name;
            }
        }
    }

    fn add_nodes(&mut self, cfg: &mut NodeConfigs<'a>) {
        cfg.foreach(&mut |cfg, _| {
            if let NodeConfigs::Single { config, .. } = cfg {
                let handle = config.handle.unwrap_or_else(|| {
                    if let Some(command) = config.command.take() {
                        let handle = NodeHandle::Command(self.commands.len());
                        self.commands.push(CommandNode {
                            command: Exclusive::new(command),
                            debug_name: String::new(),
                        });
                        handle
                    } else {
                        if let Some(tag) = config.tag.clone() {
                            if let Some(handle) = self.tags.get_dyn(tag) {
                                config.tag = None;
                                return *handle;
                            }
                        }
                        let handle = NodeHandle::Container(self.containers.len());
                        self.containers.push(ContainerNode {
                            debug_name: String::new(),
                        });
                        handle
                    }
                });
                config.handle = Some(handle);
                if let Some(tag) = config.tag.take() {
                    assert!(
                        self.tags.insert_dyn(tag.clone(), handle)
                            || handle == *self.tags.get_dyn(tag).unwrap()
                    );
                }
                if let Some(name) = config.debug_name.take() {
                    self.set_debug(handle, name);
                }
                if let Some(release) = config.release.take() {
                    self.release.push(release);
                }
            }
        });
        cfg.foreach(&mut |cfg, _| {
            cfg.constraints_mut()
                .iter_mut()
                .for_each(|(_, target)| self.add_nodes(target));
        });
    }
    fn add_constraints(&mut self, cfg: &mut NodeConfigs<'a>) -> Vec<NodeHandle> {
        cfg.foreach(&mut |cfg, children: Vec<Vec<NodeHandle>>| {
            if let NodeConfigs::Multiple { chain: true, .. } = cfg {
                for window in children.windows(2) {
                    for &node in &window[0] {
                        for &target in &window[1] {
                            self.dependency.add_edge(node, target, ());
                        }
                    }
                }
            }
            let nodes = if let NodeConfigs::Single {
                config: SingleConfig { handle, .. },
                ..
            } = cfg
            {
                vec![handle.unwrap()]
            } else {
                children.into_iter().flatten().collect()
            };
            cfg.constraints_mut()
                .iter_mut()
                .for_each(|(constraint, target)| {
                    let targets = self.add_constraints(target);
                    for &node in &nodes {
                        for &target in &targets {
                            match constraint {
                                Constraint::Before => self.dependency.add_edge(node, target, ()),
                                Constraint::After => self.dependency.add_edge(target, node, ()),
                                Constraint::Contains => self.hierarchy.add_edge(node, target, ()),
                                Constraint::Within => self.hierarchy.add_edge(target, node, ()),
                            };
                        }
                    }
                });
            nodes
        })
    }
    pub fn add_handles(&mut self, cfg: impl AsNodes<'a>) -> Vec<NodeHandle> {
        let mut cfg = cfg.into_node_configs();
        self.add_nodes(&mut cfg);
        self.add_constraints(&mut cfg)
    }
    pub fn add_single(&mut self, cfg: impl AsNodes<'a>) -> NodeHandle {
        let mut cfg = cfg.into_node_configs();
        self.add_nodes(&mut cfg);
        let nodes = self.add_constraints(&mut cfg);
        assert_eq!(nodes.len(), 1);
        nodes[0]
    }
    pub fn add(&mut self, cfg: impl AsNodes<'a>) -> &mut Self {
        let mut cfg = cfg.into_node_configs();
        self.add_nodes(&mut cfg);
        self.add_constraints(&mut cfg);
        self
    }

    pub fn name_of(&self, handle: NodeHandle) -> &str {
        match handle {
            NodeHandle::Container(idx) => self
                .containers
                .get(idx)
                .map(|x| &*x.debug_name)
                .unwrap_or(""),
            NodeHandle::Command(idx) => {
                self.commands.get(idx).map(|x| &*x.debug_name).unwrap_or("")
            }
        }
    }
}

pub enum Constraint {
    Before,
    After,
    Contains,
    Within,
}

#[derive(Default)]
pub struct SingleConfig<'a> {
    pub handle: Option<NodeHandle>,
    pub tag: Option<DynTag>,
    pub debug_name: Option<String>,
    pub command: Option<Command<'a, 'a>>,
    pub release: Option<Exclusive<Box<dyn Send>>>,
}

#[must_use]
pub enum NodeConfigs<'a> {
    Single {
        config: SingleConfig<'a>,
        constraints: Vec<(Constraint, NodeConfigs<'a>)>,
    },
    Multiple {
        configs: Vec<NodeConfigs<'a>>,
        chain: bool,
        constraints: Vec<(Constraint, NodeConfigs<'a>)>,
    },
}
impl<'a> NodeConfigs<'a> {
    fn add_constraint(&mut self, constraint: Constraint, target: NodeConfigs<'a>) {
        self.constraints_mut().push((constraint, target));
    }
    fn constraints_mut(&mut self) -> &mut Vec<(Constraint, NodeConfigs<'a>)> {
        match self {
            NodeConfigs::Single { constraints, .. } => constraints,
            NodeConfigs::Multiple { constraints, .. } => constraints,
        }
    }
    fn foreach<T>(&mut self, f: &mut impl FnMut(&mut NodeConfigs<'a>, Vec<T>) -> T) -> T {
        let mut accum = vec![];
        if let NodeConfigs::Multiple { configs, .. } = self {
            for cfg in configs {
                accum.push(cfg.foreach(f));
            }
        }
        f(self, accum)
    }
}

pub trait AsNodes<'a>: Sized {
    fn into_node_configs(self) -> NodeConfigs<'a>;
    fn before(self, other: impl AsNodes<'a>) -> NodeConfigs<'a> {
        let mut cfg = self.into_node_configs();
        cfg.add_constraint(Constraint::Before, other.into_node_configs());
        cfg
    }
    fn after(self, other: impl AsNodes<'a>) -> NodeConfigs<'a> {
        let mut cfg = self.into_node_configs();
        cfg.add_constraint(Constraint::After, other.into_node_configs());
        cfg
    }
    fn contains(self, other: impl AsNodes<'a>) -> NodeConfigs<'a> {
        let mut cfg = self.into_node_configs();
        cfg.add_constraint(Constraint::Contains, other.into_node_configs());
        cfg
    }
    fn within(self, other: impl AsNodes<'a>) -> NodeConfigs<'a> {
        let mut cfg = self.into_node_configs();
        cfg.add_constraint(Constraint::Within, other.into_node_configs());
        cfg
    }
    fn chain(self) -> NodeConfigs<'a> {
        let mut cfg = self.into_node_configs();
        let NodeConfigs::Multiple { chain, .. } = &mut cfg else {
            panic!("Cannot chain a single node.");
        };
        *chain = true;
        cfg
    }
    fn tag(self, tag: impl Tag) -> NodeConfigs<'a> {
        let mut cfg = self.into_node_configs();
        let NodeConfigs::Single {
            config: SingleConfig { tag: t, .. },
            ..
        } = &mut cfg
        else {
            panic!("Cannot tag a tuple node.");
        };
        *t = Some(DynTag::new(tag));
        cfg
    }
    fn debug(self, name: impl AsRef<str>) -> NodeConfigs<'a> {
        let mut cfg = self.into_node_configs();
        let NodeConfigs::Single {
            config: SingleConfig { debug_name, .. },
            ..
        } = &mut cfg
        else {
            panic!("Cannot name a tuple node.");
        };
        *debug_name = Some(name.as_ref().to_string());
        cfg
    }
    fn release(self, release: impl Send + 'static) -> NodeConfigs<'a> {
        let mut cfg = self.into_node_configs();
        let NodeConfigs::Single {
            config: SingleConfig {
                release: r @ None, ..
            },
            ..
        } = &mut cfg
        else {
            panic!("Cannot release a tuple node.");
        };
        *r = Some(Exclusive::new(Box::new(release)));
        cfg
    }
    fn release_fn(self, release: impl FnOnce() + Send + 'static) -> NodeConfigs<'a> {
        self.release(FnRelease::new(release))
    }
    fn execute(self, device: &Device) {
        let mut graph = ComputeGraph::new(device);
        graph.add(self);
        graph.execute();
    }
}
impl<'a> AsNodes<'a> for NodeConfigs<'a> {
    fn into_node_configs(self) -> NodeConfigs<'a> {
        self
    }
}
impl<'a, X: Tag> AsNodes<'a> for X {
    fn into_node_configs(self) -> NodeConfigs<'a> {
        NodeConfigs::Single {
            config: SingleConfig {
                tag: Some(DynTag::new(self)),
                ..Default::default()
            },
            constraints: Vec::new(),
        }
    }
}
impl<'a> AsNodes<'a> for Command<'a, 'a> {
    fn into_node_configs(self) -> NodeConfigs<'a> {
        NodeConfigs::Single {
            config: SingleConfig {
                command: Some(self),
                ..Default::default()
            },
            constraints: Vec::new(),
        }
    }
}
impl<'a> AsNodes<'a> for NodeHandle {
    fn into_node_configs(self) -> NodeConfigs<'a> {
        NodeConfigs::Single {
            config: SingleConfig {
                handle: Some(self),
                ..Default::default()
            },
            constraints: Vec::new(),
        }
    }
}

impl<'a, X> AsNodes<'a> for Vec<X>
where
    X: AsNodes<'a>,
{
    fn into_node_configs(self) -> NodeConfigs<'a> {
        NodeConfigs::Multiple {
            configs: self.into_iter().map(|x| x.into_node_configs()).collect(),
            chain: false,
            constraints: Vec::new(),
        }
    }
}

impl<'a> AsNodes<'a> for () {
    fn into_node_configs(self) -> NodeConfigs<'a> {
        NodeConfigs::Single {
            config: Default::default(),
            constraints: Vec::new(),
        }
    }
}

impl<'a> AsNodes<'a> for String {
    fn into_node_configs(self) -> NodeConfigs<'a> {
        NodeConfigs::Single {
            config: SingleConfig {
                debug_name: Some(self),
                ..Default::default()
            },
            constraints: Vec::new(),
        }
    }
}
impl<'a, 'b> AsNodes<'a> for &'b str {
    fn into_node_configs(self) -> NodeConfigs<'a> {
        NodeConfigs::Single {
            config: SingleConfig {
                debug_name: Some(self.to_string()),
                ..Default::default()
            },
            constraints: Vec::new(),
        }
    }
}

macro_rules! impl_tuple_cfgs {
    () => {};
    ($($Sn:ident),*) => {
        #[allow(non_snake_case)]
        impl<'a, $($Sn),*> AsNodes<'a> for ($($Sn,)*)
        where
            $($Sn: AsNodes<'a>),*
        {
            fn into_node_configs(self) -> NodeConfigs<'a> {
                let ($($Sn,)*) = self;
                NodeConfigs::Multiple {
                    configs: vec![$($Sn.into_node_configs()),*],
                    chain: false,
                    constraints: Vec::new(),
                }
            }
        }
        impl_tuple_cfgs!(@ $($Sn),*);
    };
    (@ $S1:ident $(, $Sn:ident)*) => {
        impl_tuple_cfgs!($($Sn),*);
    };
}
impl_tuple_cfgs!(S0, S1, S2, S3, S4, S5, S6, S7, S8, S9, S10, S11, S12, S13, S14, S15);
