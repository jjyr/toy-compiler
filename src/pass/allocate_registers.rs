use crate::ast::*;
use crate::graph::Graph;
use std::collections::{HashMap, HashSet};

const WORD: usize = 8;

#[derive(Default)]
struct Status {
    color: Option<usize>,
    conflicts: HashSet<usize>,
}

/// choose a color
fn choose_a_color(
    node: &Node,
    status: &HashMap<Node, Status>,
    move_relation: &Graph<Node>,
) -> usize {
    let node_status = status.get(node).expect("status");

    // pick a color based on move relation
    for related in move_relation.get_adjacents_set(node).expect("adjacents") {
        if let Some(s) = status.get(&related) {
            // use color of related node if it is possible
            let color = match s.color {
                Some(c) => c,
                None => continue,
            };

            if !node_status.conflicts.contains(&color) {
                return color;
            }
        }
    }

    // pick a color
    for i in 0.. {
        if !node_status.conflicts.contains(&i) {
            return i;
        }
    }

    panic!("can't choose a color")
}

fn find_most_saturated_vertex(
    status: &HashMap<Node, Status>,
    interference: &Graph<Node>,
) -> Option<Node> {
    let v = interference
        .iter_vertex()
        .filter(|v| status.get(v).expect("status").color.is_none())
        .max_by_key(|v| interference.get_adjacents_set(v).expect("adjacents").len());
    v.map(Clone::clone)
}

fn color_graph(
    interference: &mut Graph<Node>,
    move_relation: &mut Graph<Node>,
) -> HashMap<String, usize> {
    // remove RAX, since we use RAX to patch instructions,
    // so we do not allocate RAX for variables
    // which means RAX wound not be interferenced with other variables / registers
    interference.remove(&Node::RAX);

    // 1. find the most saturated vertex
    // 2. allocate a color
    // 3. mark adjacent vertexes
    let mut status: HashMap<Node, Status> = interference
        .iter_vertex()
        .cloned()
        .map(|vertex| (vertex, Status::default()))
        .collect();
    while let Some(vertex) = find_most_saturated_vertex(&status, interference) {
        let c = choose_a_color(&vertex, &status, move_relation);

        // update color
        let mut s: &mut Status = status.get_mut(&vertex).expect("vertex");
        s.color = Some(c);

        // update adjacents' conflicts
        for var in interference.get_adjacents_set(&vertex).expect("adjacents") {
            status.get_mut(&var).unwrap().conflicts.insert(c);
        }
    }

    // mapping color to registers
    status
        .into_iter()
        .map(|(node, status)| {
            (
                node.var().expect("var").to_owned(),
                status.color.expect("allocated"),
            )
        })
        .collect()
}

fn map_var_node(var_to_reg: &HashMap<String, Node>, node: Box<Node>) -> Box<Node> {
    if let Node::Var(var) = node.as_ref() {
        let value = var_to_reg[var].clone();
        Box::new(value)
    } else {
        node
    }
}

pub fn allocate_registers(node_list: Vec<Box<Node>>, info: &mut Info) -> Vec<Box<Node>> {
    use Node::*;

    let color_map = color_graph(&mut info.interference_graph, &mut info.move_graph);
    let stack_vars_count = color_map.values().max().cloned().unwrap_or(0);

    // mapping color to registers
    let var_to_reg: HashMap<String, Node> = color_map
        .into_iter()
        .map(|(var, color)| {
            let reg = match color {
                0 => RBX,
                offset => StackLoc(-((offset * WORD) as isize)),
            };
            (var, reg)
        })
        .collect();

    let mut new_node_list = Vec::with_capacity(node_list.len());
    for node in node_list {
        let node = match *node {
            ADDQ { target, arg } => {
                let target = map_var_node(&var_to_reg, target);
                let arg = map_var_node(&var_to_reg, arg);
                Box::new(ADDQ { target, arg })
            }
            MOVQ { target, source } => {
                let target = map_var_node(&var_to_reg, target);
                let source = map_var_node(&var_to_reg, source);
                Box::new(MOVQ { target, source })
            }
            value => Box::new(value),
        };
        new_node_list.push(node);
    }
    info.stack_vars_count = stack_vars_count;
    new_node_list
}
