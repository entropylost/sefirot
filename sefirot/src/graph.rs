use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::mem::transmute;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Exclusive};

use generational_arena::{Arena, Index};
use static_assertions::assert_impl_all;

use crate::element::Context;
use crate::prelude::*;

pub use self::tag::Tag;
use self::tag::TagMap;

pub mod tag;

pub trait AsNodeHandle: Copy + 'static {
    fn into_node_handle(self, graph: &ComputeGraph<'_>) -> NodeHandle;
}
impl AsNodeHandle for NodeHandle {
    fn into_node_handle(self, _graph: &ComputeGraph<'_>) -> NodeHandle {
        self
    }
}
impl<T> AsNodeHandle for T
where
    T: Tag,
{
    fn into_node_handle(self, graph: &ComputeGraph<'_>) -> NodeHandle {
        *graph.tags.get(self).unwrap()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct NodeHandle(Index);

#[derive(Debug)]
pub struct Node<'a> {
    incoming: HashSet<NodeHandle>,
    outgoing: HashSet<NodeHandle>,
    parent: Option<NodeHandle>,
    pub debug_name: String,
    data: NodeData<'a>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct FenceNode;

pub struct CommandNode<'a> {
    #[allow(dead_code)]
    pub(crate) context: Arc<Context>,
    pub command: Exclusive<Command<'a, 'a>>,
}
impl Debug for CommandNode<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CommandNode { .. }")
    }
}

#[derive(Debug, Clone)]
pub struct ContainerNode {
    pub(crate) nodes: HashSet<NodeHandle>,
}

#[derive(Debug)]
pub enum NodeData<'a> {
    Fence(FenceNode),
    Command(CommandNode<'a>),
    Container(ContainerNode),
}
impl<'a> NodeData<'a> {
    pub fn fence() -> Self {
        Self::Fence(FenceNode)
    }
    pub fn command(command: Command<'a, 'a>) -> Self {
        Self::Command(CommandNode {
            context: Arc::new(Context::new()),
            command: Exclusive::new(command),
        })
    }
    pub fn container() -> Self {
        Self::Container(ContainerNode {
            nodes: HashSet::new(),
        })
    }
    pub fn fence_ref(&self) -> Option<&FenceNode> {
        if let Self::Fence(fence) = self {
            Some(fence)
        } else {
            None
        }
    }
    pub fn fence_mut(&mut self) -> Option<&mut FenceNode> {
        if let Self::Fence(fence) = self {
            Some(fence)
        } else {
            None
        }
    }
    pub fn command_ref(&self) -> Option<&CommandNode> {
        if let Self::Command(command) = self {
            Some(command)
        } else {
            None
        }
    }
    pub fn command_mut(&mut self) -> Option<&mut CommandNode<'a>> {
        if let Self::Command(command) = self {
            Some(command)
        } else {
            None
        }
    }
    pub fn container_ref(&self) -> Option<&ContainerNode> {
        if let Self::Container(container) = self {
            Some(container)
        } else {
            None
        }
    }
    pub fn container_mut(&mut self) -> Option<&mut ContainerNode> {
        if let Self::Container(container) = self {
            Some(container)
        } else {
            None
        }
    }
}

#[cfg_attr(
    feature = "bevy",
    derive(bevy_ecs::prelude::Resource, bevy_ecs::prelude::Component)
)]
pub struct ComputeGraph<'a> {
    tags: TagMap<NodeHandle>,
    nodes: Arena<Node<'a>>,
    root: NodeHandle,
    device: Device,
    // Resources to be released after the graph is executed.
    release: Vec<Exclusive<Box<dyn Any + Send>>>,
    // Variable for storing the context.
    // SAFETY: This is only ever accessed using a mutable borrow with a lifetime less than the graph's lifetime.
    context: Option<GraphContext<'a>>,
}
assert_impl_all!(ComputeGraph: Send, Sync);
impl Debug for ComputeGraph<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComputeGraph")
            .field("tags", &self.tags)
            .field("nodes", &self.nodes)
            .field("root", &self.root)
            .finish()
    }
}
impl<'a> Deref for ComputeGraph<'a> {
    type Target = GraphContext<'a>;
    fn deref(&self) -> &Self::Target {
        unreachable!("GraphContext is only usable using `&mut`, so this should never be called.")
    }
}
impl<'a> DerefMut for ComputeGraph<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.context = Some(GraphContext {
            root: self.root,
            graph: unsafe { transmute::<&mut Self, &'a mut Self>(self) },
        });
        self.context.as_mut().unwrap()
    }
}
impl<'a> ComputeGraph<'a> {
    pub fn new(device: &Device) -> Self {
        let mut nodes = Arena::new();
        let root = NodeHandle(nodes.insert(Node {
            incoming: HashSet::new(),
            outgoing: HashSet::new(),
            parent: None,
            debug_name: "root".to_string(),
            data: NodeData::Container(ContainerNode {
                nodes: HashSet::new(),
            }),
        }));
        Self {
            tags: TagMap::new(),
            nodes,
            root,
            device: device.clone(),
            release: Vec::new(),
            context: None,
        }
    }
    pub fn root(&mut self) -> NodeRef<'_, 'a> {
        let r = self.root;
        self.on(r)
    }

    fn depth_first(&self) -> Vec<NodeHandle> {
        let mut to_visit = vec![self.root];
        let mut result = Vec::new();
        while let Some(node) = to_visit.pop() {
            result.push(node);
            if let NodeData::Container(ContainerNode { nodes }) = &self.nodes[node.0].data {
                to_visit.extend(nodes.iter().copied());
            }
        }
        result
    }
    // Removes all `Container` nodes by replacing them with Fences before and after.
    fn reduce_containers(&mut self) {
        let all_containers = self
            .depth_first()
            .into_iter()
            .filter(|node| {
                let node = &self.nodes[node.0];
                node.data.container_ref().is_some() && node.parent.is_some()
            })
            .collect::<Vec<_>>();
        for handle in all_containers {
            let container = self.remove(handle).unwrap();
            let parent = container.parent.unwrap();
            let nodes = &container.data.container_ref().unwrap().nodes;
            let start = *self.add(NodeData::fence());
            let end = *self.add(NodeData::fence());
            self.on(start)
                .before(end)
                .before_all(nodes)
                .after_all(&container.incoming)
                .parent(parent);
            self.on(end)
                .after(start)
                .after_all(nodes)
                .before_all(&container.outgoing)
                .parent(parent);
        }
    }
    fn graph(
        &self,
    ) -> (
        petgraph::Graph<NodeHandle, ()>,
        HashMap<NodeHandle, petgraph::graph::NodeIndex>,
    ) {
        let mut graph: petgraph::Graph<NodeHandle, ()> = petgraph::Graph::<NodeHandle, ()>::new();
        let mut node_map = HashMap::new();
        for handle in &self.nodes[self.root.0].data.container_ref().unwrap().nodes {
            node_map.insert(*handle, graph.add_node(*handle));
        }
        for (handle, graph_node) in &node_map {
            let node = &self.nodes[handle.0];
            for incoming in &node.incoming {
                graph.add_edge(node_map[&incoming], *graph_node, ());
            }
        }
        (graph, node_map)
    }
    // Removes edges that aren't necessary due to transitivity.
    fn reduce_transitive(&mut self) {
        use petgraph::algo::tred;
        use petgraph::visit::IntoNeighbors;
        let (graph, node_map) = self.graph();
        let order = petgraph::algo::toposort(&graph, None).unwrap();
        let (sorted_adj, revmap) = tred::dag_to_toposorted_adjacency_list::<_, u32>(&graph, &order);
        let (reduction, _closure) = tred::dag_transitive_reduction_closure::<_, u32>(&sorted_adj);

        for (handle, graph_node) in &node_map {
            let node = &mut self.nodes[handle.0];
            let mut outgoing = std::mem::take(&mut node.outgoing);
            let allowed_outgoing = reduction
                .neighbors(revmap[graph_node.index()])
                .collect::<HashSet<_>>();
            outgoing.retain(|other_node| {
                if allowed_outgoing.contains(&revmap[node_map[other_node].index()]) {
                    true
                } else {
                    self.nodes[other_node.0].incoming.remove(handle);
                    false
                }
            });
            self.nodes[handle.0].outgoing = outgoing;
        }
    }
    fn order(&self) -> Vec<NodeHandle> {
        let (graph, _node_map) = self.graph();
        let order = petgraph::algo::toposort(&graph, None).unwrap();
        order.iter().map(|i| graph[*i]).collect::<Vec<_>>()
    }
    // Replace fences with networks of connections.
    fn reduce_fences(&mut self) {
        let all_fences = self
            .nodes
            .iter()
            .filter_map(|(idx, node)| node.data.fence_ref().map(|_| NodeHandle(idx)))
            .collect::<Vec<_>>();
        for handle in all_fences {
            let fence = self.remove(handle).unwrap();
            for in_node in fence.incoming {
                self.on(in_node).before_all(&fence.outgoing);
            }
        }
    }

    fn reduce(&mut self) {
        self.reduce_containers();
        assert_eq!(
            self.nodes.len() - 1,
            self.nodes[self.root.0]
                .data
                .container_ref()
                .unwrap()
                .nodes
                .len(),
            "Graph is not connected."
        );
        self.reduce_transitive();
        self.reduce_fences();
        self.reduce_transitive();
    }

    // TODO: This currently does not parallelize anything.
    /// Consumes the graph, executing it.
    pub fn execute(mut self) {
        self.reduce();

        let order = self.order();
        let mut commands = Vec::new();
        for handle in order {
            let node = self.nodes.remove(handle.0).unwrap();
            match node.data {
                NodeData::Command(CommandNode { command, .. }) => {
                    commands.push(command.into_inner());
                }
                NodeData::Container(_) => unreachable!(),
                NodeData::Fence(_) => {}
            }
        }
        let scope = self.device.default_stream().scope();
        scope.submit_with_callback(commands, || {
            drop(self.release);
        });
    }

    /// Executes the graph without parallelism, printing debug information.
    #[cfg(feature = "debug")]
    #[tracing::instrument(skip_all, name = "ComputeGraph::execute_dbg")]
    pub fn execute_dbg(mut self) {
        use tracing::info_span;
        self.reduce();

        let order = self.order();
        for handle in order {
            let node = self.nodes.remove(handle.0).unwrap();
            match node.data {
                NodeData::Command(CommandNode { command, .. }) => {
                    let _span = info_span!("command", name = node.debug_name).entered();
                    let scope = self.device.default_stream().scope();
                    scope.submit(std::iter::once(command.into_inner()));
                }
                NodeData::Container(_) => unreachable!(),
                NodeData::Fence(_) => {}
            }
        }
    }

    #[cfg(feature = "debug")]
    pub fn execute_clear_dbg(&mut self) {
        std::mem::replace(self, Self::new(&self.device)).execute_dbg();
    }

    /// Executes the graph and clears it.
    pub fn execute_clear(&mut self) {
        std::mem::replace(self, Self::new(&self.device)).execute();
    }
}

pub struct GraphContext<'a> {
    root: NodeHandle,
    graph: &'a mut ComputeGraph<'a>,
}
impl<'a> GraphContext<'a> {
    pub fn graph(&mut self) -> &mut ComputeGraph<'a> {
        self.graph
    }
    pub fn add<'b>(&'b mut self, f: impl AsNode<'a>) -> NodeRef<'b, 'a> {
        let id = f.add(self.graph);
        let root = self.root;
        let node = self.on(id);
        node.parent(root)
    }
    pub fn fence<'b>(&'b mut self) -> NodeRef<'b, 'a> {
        self.add(NodeData::fence())
    }
    pub fn container<'b>(&'b mut self) -> NodeRef<'b, 'a> {
        self.add(NodeData::container())
    }
    pub fn head(&self) -> NodeHandle {
        self.root
    }
    pub fn device(&self) -> &Device {
        &self.graph.device
    }
    /// Removes a node, pushing its children up to its parent.
    pub fn remove(&mut self, handle: impl AsNodeHandle) -> Option<Node<'a>> {
        // TODO: Also remove the tag.
        let handle = handle.into_node_handle(self.graph);
        if let Some(node) = self.graph.nodes.remove(handle.0) {
            for incoming in &node.incoming {
                self.graph.nodes[incoming.0].outgoing.remove(&handle);
            }
            for outgoing in &node.outgoing {
                self.graph.nodes[outgoing.0].incoming.remove(&handle);
            }
            let parent = node.parent.expect("Cannot remove the root node.");

            if !self.graph.nodes.contains(parent.0) {
                panic!(
                    "Error: Cannot find parent node {:?} ({:?}) of node {:?} ({:?}).",
                    self.graph.tags.get_tag(&parent),
                    parent,
                    self.graph.tags.get_tag(&parent),
                    handle
                );
            }
            self.graph.nodes[parent.0]
                .data
                .container_mut()
                .unwrap()
                .nodes
                .remove(&handle);
            if let Some(ContainerNode { nodes }) = node.data.container_ref() {
                for child in nodes {
                    self.graph.nodes[child.0].parent = Some(parent);
                    self.graph.nodes[parent.0]
                        .data
                        .container_mut()
                        .unwrap()
                        .nodes
                        .insert(*child);
                }
            }
            Some(node)
        } else {
            None
        }
    }
    pub fn on<'b>(&'b mut self, handle: impl AsNodeHandle) -> NodeRef<'b, 'a> {
        NodeRef {
            handle: handle.into_node_handle(self.graph),
            context: self,
        }
    }
    pub fn handle(&self, tag: impl Tag) -> NodeHandle {
        *self.graph.tags.get(tag).unwrap()
    }
}

pub struct NodeRef<'b, 'a> {
    handle: NodeHandle,
    context: &'b mut GraphContext<'a>,
}
impl Deref for NodeRef<'_, '_> {
    type Target = NodeHandle;
    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}
impl<'a> NodeRef<'_, 'a> {
    pub fn tag(self, tag: impl Tag + Copy) -> Self {
        self.context.graph.tags.insert(tag, self.handle);
        self.name(format!("{:?}", tag))
    }
    pub fn name(self, name: impl AsRef<str>) -> Self {
        self.context.graph.nodes[self.handle.0].debug_name = name.as_ref().to_string();
        self
    }
    pub fn before(self, node: impl AsNodeHandle) -> Self {
        let node = node.into_node_handle(self.context.graph);
        self.context.graph.nodes[self.handle.0]
            .outgoing
            .insert(node);
        self.context.graph.nodes[node.0]
            .incoming
            .insert(self.handle);
        self
    }
    pub fn after(self, node: impl AsNodeHandle) -> Self {
        let node = node.into_node_handle(self.context.graph);
        self.context.graph.nodes[self.handle.0]
            .incoming
            .insert(node);
        self.context.graph.nodes[node.0]
            .outgoing
            .insert(self.handle);
        self
    }
    pub fn before_all<'b>(
        mut self,
        nodes: impl IntoIterator<Item = &'b impl AsNodeHandle>,
    ) -> Self {
        for node in nodes {
            self = self.before(*node);
        }
        self
    }
    pub fn after_all<'b>(mut self, nodes: impl IntoIterator<Item = &'b impl AsNodeHandle>) -> Self {
        for node in nodes {
            self = self.after(*node);
        }
        self
    }
    pub fn child(self, node: impl AsNodeHandle) -> Self {
        let node = node.into_node_handle(self.context.graph);
        self.context.on(node).detach();
        let ContainerNode { nodes } = self.context.graph.nodes[self.handle.0]
            .data
            .container_mut()
            .unwrap();
        nodes.insert(node);
        self.context.graph.nodes[node.0].parent = Some(self.handle);
        self
    }
    pub fn children<'b>(mut self, nodes: impl IntoIterator<Item = &'b impl AsNodeHandle>) -> Self {
        for node in nodes {
            self = self.child(*node);
        }
        self
    }
    pub fn children_ordered<'b>(
        mut self,
        nodes: impl IntoIterator<Item = &'b impl AsNodeHandle>,
    ) -> Self {
        let mut last_node = None;
        for node in nodes {
            if let Some(last_node) = last_node {
                self.context.on(*node).after(last_node);
            }
            self = self.child(*node);
            last_node = Some(*node);
        }
        self
    }
    pub fn detach(self) -> Self {
        if let Some(parent) = self.context.graph.nodes[self.handle.0].parent {
            self.context.graph.nodes[parent.0]
                .data
                .container_mut()
                .unwrap()
                .nodes
                .remove(&self.handle);
            self.context.graph.nodes[self.handle.0].parent = None;
        }
        self
    }
    pub fn parent(mut self, node: impl AsNodeHandle) -> Self {
        let node = node.into_node_handle(self.context.graph);
        self = self.detach();
        let ContainerNode { nodes } = self.context.graph.nodes[node.0]
            .data
            .container_mut()
            .unwrap();
        nodes.insert(self.handle);
        self.context.graph.nodes[self.handle.0].parent = Some(node);
        self
    }
    pub fn scope(self, f: impl FnOnce(&mut GraphContext<'a>)) -> Self {
        assert!(self.context.graph.nodes[self.handle.0]
            .data
            .container_ref()
            .is_some());
        let prev_root = self.context.root;
        self.context.root = self.handle;
        f(self.context);
        self.context.root = prev_root;
        self
    }
}

/// A trait representing something that can be added to a [`ComputeGraph`], returning a [`NodeHandle`]
/// for the node that was added. Note that [`add`](AddToComputeGraph::add) might add multiple nodes, as long as they're
/// all children of the return node.
pub trait AsNode<'a> {
    fn add<'b>(self, graph: &'b mut ComputeGraph<'a>) -> NodeHandle;
}
impl<'a> AsNode<'a> for NodeData<'a> {
    fn add<'b>(self, graph: &'b mut ComputeGraph<'a>) -> NodeHandle {
        NodeHandle(graph.nodes.insert(Node {
            incoming: HashSet::new(),
            outgoing: HashSet::new(),
            parent: None,
            debug_name: String::new(),
            data: self,
        }))
    }
}
impl<'a, F> AsNode<'a> for F
where
    F: for<'b> FnOnce(&'b mut ComputeGraph<'a>) -> NodeHandle,
{
    fn add<'b>(self, graph: &'b mut ComputeGraph<'a>) -> NodeHandle {
        self(graph)
    }
}
impl<'a> AsNode<'a> for Command<'a, 'a> {
    fn add<'b>(self, graph: &'b mut ComputeGraph<'a>) -> NodeHandle {
        NodeData::command(self).add(graph)
    }
}

#[cfg(feature = "copy-from")]
pub struct CopyFromBuffer<T: Value> {
    src: BufferView<T>,
    guard: tokio::sync::OwnedMutexGuard<Vec<T>>,
}
#[cfg(feature = "copy-from")]
impl<T: Value> CopyFromBuffer<T> {
    pub fn new(
        src: impl Deref<Target = BufferView<T>>,
        dst: Arc<tokio::sync::Mutex<Vec<T>>>,
    ) -> Self {
        let src = src.clone();
        // TODO: Make this lock upon initialization of the graph.
        let guard = dst.clone().blocking_lock_owned();
        Self { src, guard }
    }
}
#[cfg(feature = "copy-from")]
impl<'a, T: Value + Send> AsNode<'a> for CopyFromBuffer<T> {
    fn add<'b>(self, graph: &'b mut ComputeGraph<'a>) -> NodeHandle {
        let mut guard = self.guard;
        let dst = &mut **guard;
        let dst = unsafe { std::mem::transmute::<&mut [T], &'static mut [T]>(dst) };
        graph.release.push(Exclusive::new(Box::new(guard)));
        NodeData::command(self.src.copy_to_async(dst)).add(graph)
    }
}
