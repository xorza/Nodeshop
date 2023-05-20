use std::hint::black_box;
use std::mem;
use crate::graph::*;
use crate::node::*;

#[derive(Clone)]
struct IntermediateNode {
    pub node_id: u32,
    pub behavior: NodeBehavior,
    pub is_complete: bool,
    pub edge_behavior: EdgeBehavior,

    pub should_execute: bool,
    pub has_outputs: bool,
}

pub struct RuntimeGraph {
    nodes: Vec<IntermediateNode>,
    prev_run: Vec<IntermediateNode>,
}

impl RuntimeGraph {
    pub fn new() -> RuntimeGraph {
        RuntimeGraph {
            nodes: Vec::new(),
            prev_run: Vec::new(),
        }
    }

    pub fn run(&mut self, graph: &Graph) {
        self.traverse_backward(graph);
        self.traverse_forward1(graph);
        self.traverse_forward2(graph);

        mem::swap(&mut self.prev_run, &mut self.nodes);
        self.nodes.clear();
    }

    fn traverse_backward(&mut self, graph: &Graph) {
        self.nodes.clear();

        let active_nodes: Vec<&Node> = graph.nodes.iter().filter(|node| node.is_output).collect();
        for node in active_nodes {
            let i_node = IntermediateNode {
                node_id: node.self_id,
                behavior: NodeBehavior::Active,
                edge_behavior: EdgeBehavior::Always,
                is_complete: true,
                should_execute: false,
                has_outputs: false,
            };
            self.nodes.push(i_node);
        }

        let mut i: usize = 0;
        while i < self.nodes.len() {
            let mut i_node = self.nodes[i].clone();

            let inputs = graph.inputs_by_node_id(i_node.node_id);
            for input in inputs {
                if let Some(edge) = graph.edge_by_input_id(input.self_id) {
                    let output = graph.output_by_id(edge.output_id).unwrap();
                    let output_node = graph.node_by_id(output.node_id).unwrap();

                    let mut output_i_node: &mut IntermediateNode;
                    if let Some(_node) = self.nodes.iter_mut().find(|node| node.node_id == output_node.self_id) {
                        output_i_node = _node;
                    } else {
                        self.nodes.push(IntermediateNode {
                            node_id: output_node.self_id,
                            behavior: output_node.behavior,
                            is_complete: true,
                            edge_behavior: EdgeBehavior::Once,
                            should_execute: false,
                            has_outputs: false,
                        });
                        output_i_node = self.nodes.last_mut().unwrap();

                        if let Some(_node) = self.prev_run.iter_mut().find(|node| node.node_id == output_node.self_id) {
                            output_i_node.has_outputs = _node.has_outputs;
                        }
                    }

                    if i_node.edge_behavior == EdgeBehavior::Always
                        && edge.behavior == EdgeBehavior::Always {
                        output_i_node.edge_behavior = EdgeBehavior::Always;
                    }
                } else {
                    i_node.is_complete = false;
                }
            }

            self.nodes[i] = i_node;
            i += 1;
        }

        self.nodes.reverse();
    }

    fn traverse_forward1(&mut self, graph: &Graph) {
        for i in 0..self.nodes.len() {
            let mut i_node = self.nodes[i].clone();

            let inputs = graph.inputs_by_node_id(i_node.node_id);
            for input in inputs {
                if let Some(edge) = graph.edge_by_input_id(input.self_id) {
                    let output = graph.output_by_id(edge.output_id).unwrap();
                    let output_i_node = self.nodes.iter().find(|node| node.node_id == output.node_id).unwrap();
                    if output_i_node.is_complete == false {
                        i_node.is_complete = false;
                    }
                } else {
                    if input.is_required {
                        i_node.is_complete = false;
                    }
                }
            }

            self.nodes[i] = i_node;
        }
    }

    fn traverse_forward2(&mut self, graph: &Graph) {
        for i in 0..self.nodes.len() {
            let mut i_node = self.nodes[i].clone();
            if i_node.is_complete == false {
                continue;
            }

            if i_node.has_outputs {
                if i_node.edge_behavior == EdgeBehavior::Once {
                    continue;
                }

                if i_node.behavior == NodeBehavior::Passive {
                    let mut has_updated_inputs = false;

                    let inputs = graph.inputs_by_node_id(i_node.node_id);
                    for input in inputs {
                        if let Some(edge) = graph.edge_by_input_id(input.self_id) {
                            if edge.behavior == EdgeBehavior::Always {
                                let output = graph.output_by_id(edge.output_id).unwrap();
                                let output_execution_node =
                                    self.prev_run.iter_mut()
                                        .find(|_i_node| _i_node.node_id == output.node_id)
                                        .unwrap();

                                if output_execution_node.should_execute {
                                    has_updated_inputs = true;
                                }
                            }
                        } else {
                            assert_eq!(input.is_required, false);
                        }
                    }

                    if !has_updated_inputs {
                        continue;
                    }
                }
            }

            i_node.should_execute = true;
            i_node.has_outputs = true;
            self.nodes[i] = i_node;
        }
    }

    //if (MyDebug.IsDebug) {
    //    var node = Graph.Nodes[iNode.NodeIndex];
    //    Debug.Assert(function.Arguments.Count
    //        == Graph.Inputs.Count(_ => _.NodeIndex == node.SelfIndex)
    //        + Graph.Outputs.Count(_ => _.NodeIndex == node.SelfIndex));
    //    Debug.Assert(@delegate.Method.GetParameters().Length == function.InvokeArgumentsCount);
    //}

}