use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use generational_arena::{Arena, Index};

use crate::element::Context;
use crate::prelude::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct NodeHandle(Index);

pub struct Node<'a> {
    incoming: HashSet<NodeHandle>,
    outgoing: HashSet<NodeHandle>,
    parent: Option<NodeHandle>,
    pub data: NodeData<'a>,
}

#[non_exhaustive]
pub struct FenceNode;
pub struct CommandNode<'a> {
    #[allow(dead_code)]
    pub(crate) context: Arc<Context>,
    pub command: Command<'a, 'a>,
    pub debug_name: Option<String>,
}
pub struct ContainerNode {
    pub(crate) nodes: HashSet<NodeHandle>,
}

pub enum NodeData<'a> {
    Fence(FenceNode),
    Command(CommandNode<'a>),
    Container(ContainerNode),
}
impl<'a> NodeData<'a> {
    pub fn fence() -> Self {
        Self::Fence(FenceNode)
    }
    pub fn command(command: Command<'a, 'a>, name: Option<&str>) -> Self {
        Self::Command(CommandNode {
            context: Arc::new(Context::new()),
            command,
            debug_name: name.map(|s| s.to_string()),
        })
    }
    pub fn container<'b>(nodes: impl IntoIterator<Item = &'b NodeHandle>) -> Self {
        Self::Container(ContainerNode {
            nodes: nodes.into_iter().copied().collect(),
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

pub struct ComputeGraph<'a> {
    nodes: Arena<Node<'a>>,
    root: NodeHandle,
    // Resources to be released after the graph is executed.
    release: Vec<Box<dyn Any + Send>>,
}
impl<'a> ComputeGraph<'a> {
    pub fn new() -> Self {
        let mut nodes = Arena::new();
        let root = NodeHandle(nodes.insert(Node {
            incoming: HashSet::new(),
            outgoing: HashSet::new(),
            parent: None,
            data: NodeData::Container(ContainerNode {
                nodes: HashSet::new(),
            }),
        }));
        Self {
            nodes,
            root,
            release: Vec::new(),
        }
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
            let start = self.add(NodeData::fence()).id();
            let end = self.add(NodeData::fence()).id();
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

    pub fn execute(mut self, device: &Device) {
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
        let order = self.order();
        let mut commands = Vec::new();
        for handle in order {
            let node = self.nodes.remove(handle.0).unwrap();
            match node.data {
                NodeData::Command(CommandNode { command, .. }) => {
                    commands.push(command);
                }
                NodeData::Container(_) => unreachable!(),
                NodeData::Fence(_) => {}
            }
        }
        let scope = device.default_stream().scope();
        scope.submit_with_callback(commands, || {
            drop(self.release);
        });
    }
    pub fn add<'b>(&'b mut self, f: impl AddToComputeGraph<'a>) -> NodeRef<'b, 'a> {
        let id = f.add(self);
        let root = self.root;
        let mut node = self.on(id);
        node.parent(root);
        node
    }
    pub fn root(&self) -> NodeHandle {
        self.root
    }
    pub fn remove(&mut self, handle: NodeHandle) -> Option<Node<'a>> {
        if let Some(node) = self.nodes.remove(handle.0) {
            for incoming in &node.incoming {
                self.nodes[incoming.0].outgoing.remove(&handle);
            }
            for outgoing in &node.outgoing {
                self.nodes[outgoing.0].incoming.remove(&handle);
            }
            if let Some(parent) = node.parent {
                self.nodes[parent.0]
                    .data
                    .container_mut()
                    .unwrap()
                    .nodes
                    .remove(&handle);
            }
            Some(node)
        } else {
            None
        }
    }
    pub fn on<'b>(&'b mut self, handle: NodeHandle) -> NodeRef<'b, 'a> {
        NodeRef {
            handle,
            graph: self,
        }
    }
}

pub struct NodeRef<'b, 'a: 'b> {
    handle: NodeHandle,
    graph: &'b mut ComputeGraph<'a>,
}
impl NodeRef<'_, '_> {
    pub fn id(&self) -> NodeHandle {
        self.handle
    }
    pub fn before(&mut self, node: NodeHandle) -> &mut Self {
        self.graph.nodes[self.handle.0].outgoing.insert(node);
        self.graph.nodes[node.0].incoming.insert(self.handle);
        self
    }
    pub fn after(&mut self, node: NodeHandle) -> &mut Self {
        self.graph.nodes[self.handle.0].incoming.insert(node);
        self.graph.nodes[node.0].outgoing.insert(self.handle);
        self
    }
    pub fn before_all<'a>(&mut self, nodes: impl IntoIterator<Item = &'a NodeHandle>) -> &mut Self {
        for node in nodes {
            self.before(*node);
        }
        self
    }
    pub fn after_all<'a>(&mut self, nodes: impl IntoIterator<Item = &'a NodeHandle>) -> &mut Self {
        for node in nodes {
            self.after(*node);
        }
        self
    }
    pub fn child(&mut self, node: NodeHandle) -> &mut Self {
        self.graph.on(node).detach();
        let ContainerNode { nodes } = self.graph.nodes[self.handle.0]
            .data
            .container_mut()
            .unwrap();
        nodes.insert(node);
        self.graph.nodes[node.0].parent = Some(self.handle);
        self
    }
    pub fn children<'a>(&mut self, nodes: impl IntoIterator<Item = &'a NodeHandle>) -> &mut Self {
        for node in nodes {
            self.child(*node);
        }
        self
    }
    pub fn detach(&mut self) -> &mut Self {
        if let Some(parent) = self.graph.nodes[self.handle.0].parent {
            self.graph.nodes[parent.0]
                .data
                .container_mut()
                .unwrap()
                .nodes
                .remove(&self.handle);
            self.graph.nodes[self.handle.0].parent = None;
        }
        self
    }
    pub fn parent(&mut self, node: NodeHandle) -> &mut Self {
        self.detach();
        let ContainerNode { nodes } = self.graph.nodes[node.0].data.container_mut().unwrap();
        nodes.insert(self.handle);
        self.graph.nodes[self.handle.0].parent = Some(node);
        self
    }
}

pub trait AddToComputeGraph<'a> {
    fn add<'b>(self, graph: &'b mut ComputeGraph<'a>) -> NodeHandle;
}
impl<'a> AddToComputeGraph<'a> for NodeData<'a> {
    fn add<'b>(self, graph: &'b mut ComputeGraph<'a>) -> NodeHandle {
        NodeHandle(graph.nodes.insert(Node {
            incoming: HashSet::new(),
            outgoing: HashSet::new(),
            parent: None,
            data: self,
        }))
    }
}
impl<'a, F> AddToComputeGraph<'a> for F
where
    F: for<'b> FnOnce(&'b mut ComputeGraph<'a>) -> NodeHandle,
{
    fn add<'b>(self, graph: &'b mut ComputeGraph<'a>) -> NodeHandle {
        self(graph)
    }
}
impl<'a> AddToComputeGraph<'a> for Command<'a, 'a> {
    fn add<'b>(self, graph: &'b mut ComputeGraph<'a>) -> NodeHandle {
        NodeData::command(self, None).add(graph)
    }
}

#[cfg(feature = "copy-from")]
pub struct CopyFromBuffer<'c, T: Value> {
    src: BufferView<'c, T>,
    guard: tokio::sync::OwnedMutexGuard<Vec<T>>,
}
#[cfg(feature = "copy-from")]
impl<'c, T: Value> CopyFromBuffer<'c, T> {
    pub fn new(src: &'c Buffer<T>) -> (Self, Arc<tokio::sync::Mutex<Vec<T>>>) {
        let src = src.view(..);
        Self::new_view(src)
    }
    pub fn new_view(src: BufferView<'c, T>) -> (Self, Arc<tokio::sync::Mutex<Vec<T>>>) {
        let dst = Arc::new(tokio::sync::Mutex::new(Vec::with_capacity(src.len())));
        let guard = dst.clone().blocking_lock_owned();
        (Self { src, guard }, dst)
    }
}
#[cfg(feature = "copy-from")]
impl<'a, 'c, T: Value + Send> AddToComputeGraph<'a> for CopyFromBuffer<'c, T> {
    fn add<'b>(self, graph: &'b mut ComputeGraph<'a>) -> NodeHandle {
        let mut guard = self.guard;
        let dst = &mut **guard;
        let dst = unsafe { std::mem::transmute::<&mut [T], &'static mut [T]>(dst) };
        graph.release.push(Box::new(guard));
        NodeData::command(self.src.copy_to_async(dst), None).add(graph)
    }
}
