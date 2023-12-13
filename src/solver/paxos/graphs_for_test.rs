use super::*;
use crate::set_utils::FromU128;
use scuttlebutt::field::{F128b, FiniteField as FF};
use std::fmt;

fn field2u32<F: FF>(f: F) -> u32 {
    let a = f.to_bytes();
    u32::from_ne_bytes([a[0], a[1], a[2], a[3]])
}

impl fmt::Display for Node<F128b> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dirs = self
            .dirs
            .iter()
            .map(|dir| dir.to.upgrade().unwrap().borrow().id)
            .collect::<Vec<_>>();

        write!(
            f,
            "Node {{ id: {}, visit_status: {:?}, dir_to: {:?} }}",
            self.id, self.visit_status, dirs,
        )
    }
}

impl fmt::Display for Edge<F128b> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Edge {{ point: {:?}, visit_status: {:?}, back_edge: {} }}",
            self.point, self.visit_status, self.back_edge,
        )
    }
}

#[allow(dead_code)]
pub(crate) fn graph_analyze(
    graph: &Vec<Rc<RefCell<Node<F128b>>>>,
    include_mermaid: bool,
    verbose: bool,
) -> (String, Vec<Rc<RefCell<Node<F128b>>>>) {
    if verbose {
        println!("graph: {:?}\n", graph);
    }

    enum DfsRes {
        Ok,
        BackEdge,
    }

    fn dfs_rec(
        node: Rc<RefCell<Node<F128b>>>,
        edge_count: &mut usize,
        back_edge_count: &mut usize,
        mermaid_str: &mut Option<String>,
        verbose: bool,
    ) -> DfsRes {
        if node.borrow().is_visited(FindConstraints) {
            if verbose {
                println!("Already visited node: {}\n", node.borrow());
                println!("IT IS BACK EDGE.\n");
            }
            *back_edge_count += 1;
            return DfsRes::BackEdge;
        }

        if verbose {
            println!("node: {}\n", node.borrow());
        }

        node.borrow_mut().visit_status = VisitedOnce;

        let id = node.borrow().id;
        let node_str = format!("{}((\"{}\"))", id, id);

        for (next_node, next_edge) in node.borrow().next_dirs(FindConstraints) {
            // follow the case of the edge is self loop
            if next_edge.borrow().is_visited(FindConstraints) {
                if verbose {
                    println!("It would be self loop: {}\n", next_edge.borrow());
                }
                continue;
            }

            *edge_count += 1;

            let next_id = next_node.borrow().id;
            let next_node_str = format!("{}((\"{}\"))", next_id, next_id);

            next_edge.borrow_mut().visit_status = VisitedOnce;
            let res = dfs_rec(next_node, edge_count, back_edge_count, mermaid_str, verbose);

            let edge_str_parts = match res {
                DfsRes::BackEdge => {
                    next_edge.borrow_mut().back_edge = true;
                    "-."
                }
                _ => "--",
            };

            let point_str = {
                let ne = next_edge.borrow();
                let x = field2u32(ne.point.0) % 100;
                let y = field2u32(ne.point.1) % 100;
                format!("({}, {})", x, y)
            };

            let edge_str = format!(
                "    {}{}\"{}\"{}-{}\n",
                node_str, edge_str_parts, point_str, edge_str_parts, next_node_str
            );

            if let Some(s) = mermaid_str.as_mut() {
                s.push_str(&edge_str);
            }
        }

        DfsRes::Ok
    }

    let mut mermaid_str = if include_mermaid {
        Some(String::new())
    } else {
        None
    };
    let node_num = graph.len();
    let mut edge_count = 0;
    let mut back_edge_count = 0;
    let mut new_graph = Vec::with_capacity(graph.len());
    for node in graph.iter() {
        if node.borrow().is_visited(FindConstraints) {
            if verbose {
                println!("(Already visited node: {})\n", node.borrow());
            }

            continue;
        }

        new_graph.push(Rc::clone(node));

        let _ = dfs_rec(
            Rc::clone(node),
            &mut edge_count,
            &mut back_edge_count,
            &mut mermaid_str,
            verbose,
        );
    }

    let mermaid_str = match mermaid_str {
        Some(s) => {
            let mut mermaid_str = String::new();
            mermaid_str.push_str("graph LR;\n");
            mermaid_str.push_str(&s);
            mermaid_str
        }
        None => String::new(),
    };

    let analyzed_result = format!(
        "\
back edge count / edge num = {} / {}
root node num = {} / {}

{}
",
        back_edge_count,
        edge_count,
        new_graph.len(),
        node_num,
        mermaid_str
    );

    if verbose {
        println!("\n====================\n");
    }

    (analyzed_result, new_graph)
}

pub(crate) fn create_specific_graph_empty(
    _params: PaxosSolverParams,
) -> Vec<Rc<RefCell<Node<F128b>>>> {
    Vec::new()
}

fn new_node(id: usize, params: PaxosSolverParams) -> Rc<RefCell<Node<F128b>>> {
    Rc::new(RefCell::new(Node {
        id,
        dirs: Vec::new(),
        visit_status: NotVisited,
        accumulator: CP::zero(params.r_size),
    }))
}

fn new_edge_with_point(
    point: (F128b, F128b),
    n0: Rc<RefCell<Node<F128b>>>,
    n1: Rc<RefCell<Node<F128b>>>,
) -> Rc<RefCell<Edge<F128b>>> {
    let res = Rc::new(RefCell::new(Edge {
        point,
        visit_status: NotVisited,
        back_edge: false,
    }));

    n0.borrow_mut().dirs.push(DirectTo {
        to: Rc::downgrade(&n1),
        edge: Rc::clone(&res),
    });
    n1.borrow_mut().dirs.push(DirectTo {
        to: Rc::downgrade(&n0),
        edge: Rc::clone(&res),
    });

    res
}

fn new_edge(
    id: usize,
    n0: Rc<RefCell<Node<F128b>>>,
    n1: Rc<RefCell<Node<F128b>>>,
) -> Rc<RefCell<Edge<F128b>>> {
    let point = (F128b::from_u128(id as u128), F128b::from_u128(id as u128));
    new_edge_with_point(point, n0, n1)
}

pub(crate) fn create_specific_graph_no_edge(
    params: PaxosSolverParams,
    node_num: usize,
) -> Vec<Rc<RefCell<Node<F128b>>>> {
    (0..node_num)
        .map(|i| new_node(i, params))
        .collect::<Vec<_>>()
}

pub(crate) fn create_specific_graph_self_loop(
    params: PaxosSolverParams,
    node_num: usize,
) -> Vec<Rc<RefCell<Node<F128b>>>> {
    (0..node_num)
        .map(|i| {
            let node = new_node(i, params);

            let x = F128b::from_u128(i as u128);
            let y = F128b::from_u128(i as u128);

            let edge = Rc::new(RefCell::new(Edge {
                point: (x, y),
                visit_status: NotVisited,
                back_edge: false,
            }));

            node.borrow_mut().dirs = vec![DirectTo {
                to: Rc::downgrade(&node),
                edge: Rc::clone(&edge),
            }];

            node
        })
        .collect::<Vec<_>>()
}

pub(crate) fn create_specific_graph_straight(
    params: PaxosSolverParams,
    node_num: usize,
) -> Vec<Rc<RefCell<Node<F128b>>>> {
    let mut result = Vec::new();
    let mut pre_node = new_node(0, params);
    result.push(Rc::clone(&pre_node));

    for i in 1..node_num {
        let now_node = new_node(i, params);
        result.push(Rc::clone(&now_node));

        let _ = new_edge(i - 1, Rc::clone(&pre_node), Rc::clone(&now_node));

        pre_node = now_node;
    }

    result
}

pub(crate) fn create_specific_graph_discrete_straight(
    params: PaxosSolverParams,
    node_num: usize,
) -> Vec<Rc<RefCell<Node<F128b>>>> {
    let mut result = Vec::new();
    let mut pre_node = new_node(0, params);
    result.push(Rc::clone(&pre_node));

    for i in 1..node_num {
        let now_node = new_node(i, params);
        result.push(Rc::clone(&now_node));

        if i % 2 == 1 {
            let _ = new_edge(i - 1, Rc::clone(&pre_node), Rc::clone(&now_node));
        }

        pre_node = now_node;
    }

    result
}

pub(crate) fn create_specific_graph_straight_with_self_loop(
    params: PaxosSolverParams,
    node_num: usize,
) -> Vec<Rc<RefCell<Node<F128b>>>> {
    let mut result = Vec::new();
    let mut pre_node = new_node(0, params);
    result.push(Rc::clone(&pre_node));

    let mut edge_id: u128 = 0;

    let x = F128b::from_u128(edge_id);
    let y = F128b::from_u128(edge_id);
    edge_id += 1;

    let self_loop_edge = Rc::new(RefCell::new(Edge {
        point: (x, y),
        visit_status: NotVisited,
        back_edge: false,
    }));

    pre_node.borrow_mut().dirs.push(DirectTo {
        to: Rc::downgrade(&pre_node),
        edge: Rc::clone(&self_loop_edge),
    });

    for i in 1..node_num {
        let now_node = new_node(i, params);
        result.push(Rc::clone(&now_node));

        let x = F128b::from_u128(edge_id);
        let y = F128b::from_u128(edge_id);
        edge_id += 1;

        let self_loop_edge = Rc::new(RefCell::new(Edge {
            point: (x, y),
            visit_status: NotVisited,
            back_edge: false,
        }));

        now_node.borrow_mut().dirs.push(DirectTo {
            to: Rc::downgrade(&now_node),
            edge: Rc::clone(&self_loop_edge),
        });

        let _ = new_edge(edge_id as usize, Rc::clone(&pre_node), Rc::clone(&now_node));

        edge_id += 1;

        pre_node = now_node;
    }

    result
}

pub(crate) fn create_specific_graph_straight_with_back_edges(
    params: PaxosSolverParams,
    node_num: usize,
) -> Vec<Rc<RefCell<Node<F128b>>>> {
    let mut result = Vec::new();
    let mut pre_node = new_node(0, params);

    result.push(Rc::clone(&pre_node));

    let mut edge_id = 0;
    for i in 1..node_num {
        let now_node = new_node(i, params);
        result.push(Rc::clone(&now_node));

        for _ in 0..2 {
            let _ = new_edge(edge_id, Rc::clone(&pre_node), Rc::clone(&now_node));

            edge_id += 1;
        }

        pre_node = now_node;
    }

    result
}

pub(crate) fn create_big_dipper_graph(params: PaxosSolverParams) -> Vec<Rc<RefCell<Node<F128b>>>> {
    let nodes = (0..7).map(|i| new_node(i, params)).collect::<Vec<_>>();

    let _ = new_edge(0, Rc::clone(&nodes[0]), Rc::clone(&nodes[1]));
    let _ = new_edge(1, Rc::clone(&nodes[1]), Rc::clone(&nodes[2]));
    let _ = new_edge(2, Rc::clone(&nodes[2]), Rc::clone(&nodes[3]));
    let _ = new_edge(3, Rc::clone(&nodes[3]), Rc::clone(&nodes[4]));
    let _ = new_edge(4, Rc::clone(&nodes[3]), Rc::clone(&nodes[5]));
    let _ = new_edge(5, Rc::clone(&nodes[5]), Rc::clone(&nodes[6]));
    let _ = new_edge(6, Rc::clone(&nodes[6]), Rc::clone(&nodes[4]));

    nodes
}

pub(crate) fn create_triangle_graph(params: PaxosSolverParams) -> Vec<Rc<RefCell<Node<F128b>>>> {
    let nodes = (0..3).map(|i| new_node(i, params)).collect::<Vec<_>>();

    let _ = new_edge(0, Rc::clone(&nodes[0]), Rc::clone(&nodes[1]));
    let _ = new_edge(1, Rc::clone(&nodes[1]), Rc::clone(&nodes[2]));
    let _ = new_edge(2, Rc::clone(&nodes[2]), Rc::clone(&nodes[0]));

    nodes
}

// Double Constraints

pub(crate) fn create_double_constraints_graph_0(
    params: PaxosSolverParams,
) -> Vec<Rc<RefCell<Node<F128b>>>> {
    let nodes = (0..5).map(|i| new_node(i, params)).collect::<Vec<_>>();

    let point = (F128b::from_u128(0), F128b::from_u128(10));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[0]), Rc::clone(&nodes[1]));

    let point = (
        F128b::from_u128(1),
        F128b::from_u128(1) + F128b::from_u128(2) + F128b::from_u128(100),
    );
    let _ = new_edge_with_point(point, Rc::clone(&nodes[1]), Rc::clone(&nodes[2]));

    let point = (F128b::from_u128(2), F128b::from_u128(1));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[1]), Rc::clone(&nodes[3]));

    let point = (F128b::from_u128(3), F128b::from_u128(2));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[3]), Rc::clone(&nodes[2]));

    let point = (F128b::from_u128(4), F128b::from_u128(1));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[2]), Rc::clone(&nodes[4]));

    let point = (F128b::from_u128(5), F128b::from_u128(2));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[4]), Rc::clone(&nodes[1]));

    nodes
}

pub(crate) fn create_double_constraints_graph_1(
    params: PaxosSolverParams,
) -> Vec<Rc<RefCell<Node<F128b>>>> {
    let nodes = (0..4).map(|i| new_node(i, params)).collect::<Vec<_>>();

    let point = (
        F128b::from_u128(0),
        F128b::from_u128(1) + F128b::from_u128(2) + F128b::from_u128(100),
    );
    let _ = new_edge_with_point(point, Rc::clone(&nodes[0]), Rc::clone(&nodes[1]));

    let point = (F128b::from_u128(1), F128b::from_u128(1));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[0]), Rc::clone(&nodes[2]));

    let point = (F128b::from_u128(2), F128b::from_u128(2));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[2]), Rc::clone(&nodes[1]));

    let point = (F128b::from_u128(3), F128b::from_u128(1));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[1]), Rc::clone(&nodes[3]));

    let point = (F128b::from_u128(4), F128b::from_u128(2));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[3]), Rc::clone(&nodes[0]));

    nodes
}

pub(crate) fn create_multi_constraints_graph(
    params: PaxosSolverParams,
) -> Vec<Rc<RefCell<Node<F128b>>>> {
    let nodes = (0..2).map(|i| new_node(i, params)).collect::<Vec<_>>();

    for i in 0..4 {
        let point = (F128b::from_u128(i), F128b::from_u128(10));
        let _ = new_edge_with_point(point, Rc::clone(&nodes[0]), Rc::clone(&nodes[1]));
    }

    nodes
}

pub(crate) fn create_bird_graph(params: PaxosSolverParams) -> Vec<Rc<RefCell<Node<F128b>>>> {
    let nodes = (0..8).map(|i| new_node(i, params)).collect::<Vec<_>>();

    let point = (
        F128b::from_u128(0),
        F128b::from_u128(3) + F128b::from_u128(4) + F128b::from_u128(100),
    );
    let _ = new_edge_with_point(point, Rc::clone(&nodes[0]), Rc::clone(&nodes[2]));

    let point = (F128b::from_u128(1), F128b::from_u128(3));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[2]), Rc::clone(&nodes[1]));

    let point = (F128b::from_u128(2), F128b::from_u128(4));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[1]), Rc::clone(&nodes[0]));

    let point = (F128b::from_u128(3), F128b::from_u128(10));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[2]), Rc::clone(&nodes[3]));

    let point = (
        F128b::from_u128(4),
        F128b::from_u128(5) + F128b::from_u128(6) + F128b::from_u128(200),
    );
    let _ = new_edge_with_point(point, Rc::clone(&nodes[3]), Rc::clone(&nodes[4]));

    let point = (F128b::from_u128(5), F128b::from_u128(5));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[3]), Rc::clone(&nodes[5]));

    let point = (F128b::from_u128(6), F128b::from_u128(6));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[3]), Rc::clone(&nodes[6]));

    let point = (F128b::from_u128(7), F128b::from_u128(5));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[3]), Rc::clone(&nodes[7]));

    let point = (F128b::from_u128(8), F128b::from_u128(6));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[5]), Rc::clone(&nodes[4]));

    let point = (F128b::from_u128(9), F128b::from_u128(5));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[6]), Rc::clone(&nodes[4]));

    let point = (F128b::from_u128(10), F128b::from_u128(6));
    let _ = new_edge_with_point(point, Rc::clone(&nodes[7]), Rc::clone(&nodes[4]));

    nodes
}
