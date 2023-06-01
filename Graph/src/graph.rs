use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::data_type::DataType;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum NodeBehavior {
    Active,
    Passive,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Node {
    self_id: Uuid,

    pub name: String,
    pub behavior: NodeBehavior,
    pub is_output: bool,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<Input>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<Output>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subgraph_id: Option<Uuid>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Output {
    pub name: String,
    pub data_type: DataType,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Debug)]
pub enum BindingBehavior {
    #[default]
    Always,
    Once,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Binding {
    output_node_id: Uuid,
    output_index: u32,
    pub behavior: BindingBehavior,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Input {
    pub name: String,
    pub data_type: DataType,
    pub is_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<Binding>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Argument {
    pub node_id: Uuid,
    pub arg_index: u32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SubGraph {
    self_id: Uuid,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<Argument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<Argument>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Graph {
    nodes: Vec<Node>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    subgraphs: Vec<SubGraph>,
}


impl Graph {
    pub fn new() -> Graph {
        Graph {
            nodes: Vec::new(),
            subgraphs: Vec::new(),
        }
    }

    pub fn nodes(&self) -> &Vec<Node> {
        &self.nodes
    }
    pub fn nodes_mut(&mut self) -> Vec<&mut Node> {
        self.nodes.iter_mut().collect()
    }

    pub fn add_node(&mut self, node: &mut Node) {
        if let Some(existing_node) = self.node_by_id_mut(node.id()) {
            *existing_node = node.clone();
        } else {
            self.nodes.push(node.clone());
        }
    }
    pub fn remove_node_by_id(&mut self, id: Uuid) {
        assert_ne!(id, Uuid::nil());

        self.nodes.retain(|node| node.self_id != id);

        self.nodes.iter_mut().flat_map(|node| node.inputs.iter_mut())
            .filter(|input| input.binding.is_some() && input.binding.as_ref().unwrap().output_node_id == id)
            .for_each(|input| {
                input.binding = None;
            });
    }

    pub fn node_by_name(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.name == name)
    }
    pub fn node_by_name_mut(&mut self, name: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|node| node.name == name)
    }

    pub fn node_by_id(&self, id: Uuid) -> Option<&Node> {
        if id == Uuid::nil() {
            return None;
        }
        self.nodes.iter().find(|node| node.self_id == id)
    }
    pub fn node_by_id_mut(&mut self, id: Uuid) -> Option<&mut Node> {
        if id == Uuid::nil() {
            return None;
        }
        self.nodes.iter_mut().find(|node| node.self_id == id)
    }

    pub fn to_yaml(&self) -> anyhow::Result<String> {
        let yaml = serde_yaml::to_string(&self)?;
        Ok(yaml)
    }
    pub fn from_yaml_file(path: &str) -> anyhow::Result<Graph> {
        let yaml = std::fs::read_to_string(path)?;
        let graph: Graph = serde_yaml::from_str(&yaml)?;

        graph.validate()?;

        Ok(graph)
    }
    pub fn from_yaml(yaml: &str) -> anyhow::Result<Graph> {
        let graph: Graph = serde_yaml::from_str(yaml)?;

        graph.validate()?;

        Ok(graph)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        for node in self.nodes.iter() {
            if node.self_id == Uuid::nil() {
                return Err(anyhow::Error::msg("Node has invalid id"));
            }

            for input in node.inputs.iter() {
                if let Some(binding) = &input.binding {
                    if self.node_by_id(binding.output_node_id).is_none() {
                        return Err(anyhow::Error::msg("Node has invalid binding"));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn add_subgraph(&mut self, subgraph: &mut SubGraph) {
        if let Some(existing_subgraph) = self.subgraph_by_id_mut(subgraph.self_id) {
            *existing_subgraph = subgraph.clone();
        } else {
            self.subgraphs.push(subgraph.clone());
        }
    }
    pub fn remove_subgraph_by_id(&mut self, id: Uuid) {
        assert_ne!(id, Uuid::nil());

        self.subgraphs.retain(|subgraph| subgraph.self_id != id);
        self.nodes.iter()
            .filter(|node| node.subgraph_id == Some(id))
            .map(|node| node.self_id)
            .collect::<Vec<Uuid>>()
            .iter()
            .cloned()
            .for_each(|node_id| {
                self.remove_node_by_id(node_id);
            });
    }

    pub fn subgraph_by_id_mut(&mut self, id: Uuid) -> Option<&mut SubGraph> {
        self.subgraphs.iter_mut().find(|subgraph| subgraph.self_id == id)
    }
    pub fn subgraph_by_id(&self, id: Uuid) -> Option<&SubGraph> {
        self.subgraphs.iter().find(|subgraph| subgraph.self_id == id)
    }
}

impl Node {
    pub fn new() -> Node {
        Node {
            self_id: Uuid::new_v4(),
            name: String::new(),
            behavior: NodeBehavior::Active,
            is_output: false,
            inputs: Vec::new(),
            outputs: Vec::new(),
            subgraph_id: None,
        }
    }

    pub fn id(&self) -> Uuid {
        self.self_id
    }
}

impl Input {
    pub fn new() -> Input {
        Input {
            binding: None,
            name: String::new(),
            data_type: DataType::None,
            is_required: false,
        }
    }
}

impl Output {
    pub fn new() -> Output {
        Output {
            name: String::new(),
            data_type: DataType::None,
        }
    }
}

impl Binding {
    pub fn output_node_id(&self) -> Uuid {
        self.output_node_id
    }
    pub fn output_index(&self) -> u32 {
        self.output_index
    }

    pub fn new(node_id: Uuid, output_index: u32) -> Binding {
        Binding {
            output_node_id: node_id,
            output_index,
            behavior: BindingBehavior::Always,
        }
    }
}

impl SubGraph {
    pub fn new() -> SubGraph {
        SubGraph {
            self_id: Uuid::new_v4(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    pub fn id(&self) -> Uuid {
        self.self_id
    }
}
