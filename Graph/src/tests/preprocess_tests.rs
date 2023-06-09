use crate::functions::FunctionId;
use crate::graph::*;
use crate::invoke::{InvokeArgs, Invoker};
use crate::preprocess::Preprocess;
use crate::runtime_graph::{InvokeContext, RuntimeGraph};

struct EmptyInvoker {}

impl Invoker for EmptyInvoker {
    fn all_functions(&self) -> Vec<FunctionId> {
        vec![]
    }

    fn invoke(&self,
              _function_id: FunctionId,
              _ctx: &mut InvokeContext,
              _inputs: &InvokeArgs,
              _outputs: &mut InvokeArgs)
        -> anyhow::Result<()> {
        Ok(())
    }
}


#[test]
fn simple_run() -> anyhow::Result<()> {
    let graph = Graph::from_yaml_file("../test_resources/test_graph.yml")?;
    let runtime = Preprocess::default();

    let runtime_graph = runtime.run(&graph, &mut RuntimeGraph::default());
    assert_eq!(runtime_graph.nodes.len(), 5);
    assert_eq!(runtime_graph.node_by_name("val2").unwrap().total_binding_count, 2);
    assert!(runtime_graph.nodes.iter().all(|r_node| r_node.should_execute));
    assert!(runtime_graph.nodes.iter().all(|r_node| !r_node.has_missing_inputs));

    let _yaml = serde_yaml::to_string(&runtime_graph)?;

    Ok(())
}

#[test]
fn missing_input() -> anyhow::Result<()> {
    let mut graph = Graph::from_yaml_file("../test_resources/test_graph.yml")?;
    graph.node_by_name_mut("sum").unwrap()
        .inputs[0].binding = Binding::None;

    let runtime = Preprocess::default();
    let runtime_graph = runtime.run(&graph, &mut RuntimeGraph::default());
    assert_eq!(runtime_graph.nodes.len(), 4);
    assert_eq!(runtime_graph.node_by_name("val2").unwrap().total_binding_count, 2);
    assert!(runtime_graph.nodes.iter().all(|r_node| r_node.should_execute));
    assert!(!runtime_graph.node_by_name("val2").unwrap().has_missing_inputs);
    assert!(runtime_graph.node_by_name("sum").unwrap().has_missing_inputs);
    assert!(runtime_graph.node_by_name("mult").unwrap().has_missing_inputs);
    assert!(runtime_graph.node_by_name("print").unwrap().has_missing_inputs);

    let _yaml = serde_yaml::to_string(&runtime_graph)?;

    Ok(())
}
