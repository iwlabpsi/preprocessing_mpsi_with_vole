//! # paramaters
//! - n
//! - d = log n or 1.01 * log n // 1.01 woule be a proper one for 1 + \epsilon
//! - \lambda = 40 // this would be proper one for F128b or set size 2^20.
//!
//! ## DFS based
//!
//! - m = (2.01 * n) + (d + \lambda) // 2.01 woule be a proper one for 2 + \epsilon
//! - d = log n
//! - |L| = m' = 2.01 * n
//! - |R| = d + \lambda
//!
//! ## 2-core based
//!
//! - m = (2.4n) + (d + \lambda)
//! - d = 1.01 * log n
//! - |L| = m' = 2.4n
//! - |R| = d + \lambda
//!
//! This solver is DFS based one.
//!
//! So we use m = (2.01 * n) + (log n + 40)
//!
//! See the appendix B and figure 7 in full version of "PSI from PaXoS: Fast, Malicious Private Set Intersection"
//! @ <https://eprint.iacr.org/2020/193>

use super::*;
use anyhow::{bail, Context, Result};
use gaussian_eliminations::gaussian_elimination;
use rand::distributions::{Distribution, Standard};
use rand::{CryptoRng, Rng};
use scuttlebutt::field::FiniteField as FF;
use scuttlebutt::AbstractChannel;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

// H_i: key x F -> [m]
#[inline]
pub fn hash2index<F: FF>(k: u64, x: F, max: usize) -> usize {
    let mut hasher = Sha256::new();
    hasher.update(k.to_be_bytes());
    hasher.update(x.to_bytes());
    let res = hasher.finalize();
    let res = res.as_slice();
    let res = res[0..8].try_into().unwrap();
    let res = u64::from_be_bytes(res);
    (res as usize) % max
}

// r: key x F -> {0, 1}^r_size
#[inline]
pub fn r<F: FF>(k: u64, x: F, m: usize) -> Vec<bool> {
    let mut hasher = Sha256::new();
    hasher.update(k.to_be_bytes());
    hasher.update(x.to_bytes());
    let res = hasher.finalize();

    res.iter()
        .flat_map(|&byte| (0..8).map(move |i| byte & (1 << i) != 0))
        .take(m)
        .collect()
}

fn calc_r_inner_product<F: FF>(x: F, vec_r: &[F], k3: u64, r_size: usize) -> F {
    let bits = r(k3, x, r_size);

    let mut sum = F::zero();
    for (i, b) in bits.iter().enumerate() {
        if *b {
            sum += vec_r[i];
        }
    }

    sum
}

pub struct PaxosSolver<F>(PhantomData<F>)
where
    F: FF,
    Standard: Distribution<F>;

#[derive(Clone, Copy)]
pub struct PaxosSolverParams {
    l_size: usize,
    r_size: usize,
}

impl SolverParams for PaxosSolverParams {
    fn code_length(&self) -> usize {
        self.l_size + self.r_size
    }
}

impl<F> Solver<F> for PaxosSolver<F>
where
    F: FF,
    Standard: Distribution<F>,
{
    type AuxInfo = (u64, u64, u64);
    type Params = PaxosSolverParams;

    fn gen_aux<RNG: CryptoRng + Rng>(rng: &mut RNG) -> Result<Self::AuxInfo> {
        let k1 = rng.gen::<u64>();
        let k2 = rng.gen::<u64>();
        let k3 = rng.gen::<u64>();

        Ok((k1, k2, k3))
    }

    fn aux_send<C: AbstractChannel, RNG: CryptoRng + Rng>(
        channel: &mut C,
        _rng: &mut RNG,
        aux: Self::AuxInfo,
    ) -> Result<()> {
        let (k1, k2, k3) = aux;
        channel
            .write_u64(k1)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;
        channel
            .write_u64(k2)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;
        channel
            .write_u64(k3)
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        Ok(())
    }

    fn aux_receive<C: AbstractChannel, RNG: CryptoRng + Rng>(
        channel: &mut C,
        _rng: &mut RNG,
    ) -> Result<Self::AuxInfo> {
        let k1 = channel
            .read_u64()
            .with_context(|| format!("@{}:{}", file!(), line!()))?;
        let k2 = channel
            .read_u64()
            .with_context(|| format!("@{}:{}", file!(), line!()))?;
        let k3 = channel
            .read_u64()
            .with_context(|| format!("@{}:{}", file!(), line!()))?;

        Ok((k1, k2, k3))
    }

    fn calc_params(n: usize) -> PaxosSolverParams {
        let l_size = 2 * n + n / 100;
        let logn = n.next_power_of_two().trailing_zeros() as usize;
        let r_size = logn + 40;

        PaxosSolverParams { l_size, r_size }
    }

    fn encode<RNG: CryptoRng + Rng>(
        rng: &mut RNG,
        points: &[(F, F)],
        aux: (u64, u64, u64),
        params: Self::Params,
    ) -> Result<Vec<F>> {
        // 1. Construct the Cuckoo graph $G_{h_1, h_2, X}$ for $X = \{x_1, \ldots, x_n\}$.
        let graph = construct_cuckoo_graph(points, aux, params);

        // 2. Initialize variables $L$ and $R$ and an initialliy empty set of linear constraints S.
        let mut vec_l: Vec<F> = (0..params.l_size).map(|_| rng.gen()).collect::<Vec<_>>();
        let mut vec_r: Vec<F> = (0..params.r_size).map(|_| rng.gen()).collect::<Vec<_>>();

        // 3. Perform a DFS on $G_{h_1, h_2, X}$.
        let (constraints, graph) = dfs_to_find_constraints(&graph, aux, params);

        if constraints.len() > params.r_size {
            bail!("too many constraints");
        }

        if constraints.len() > 0 {
            // 4. Solve for variables R satisfying the constraints of system S.
            let equations_w = gaussian_elimination(constraints).with_context(|| {
                format!("error in gaussian_elimination at @{}:{}", file!(), line!())
            })?;
            let Some(equations) = equations_w else {
                bail!("failed to solve linear equations");
            };

            // 4'. Adjust vec_r by equations.
            adjust_vec_r(&equations, &mut vec_r);
        }

        // 5. Perform another DFS on $G_{h_1, h_2, X}$ to compute $L$.
        dfs_to_calc_vec_l(&graph, aux, params, &vec_r, &mut vec_l);

        // 6. Output D = L || R
        let mut result = vec_l;
        result.extend(vec_r);

        Ok(result)
    }

    fn decode(p: &[F], x: F, aux: (u64, u64, u64), params: Self::Params) -> Result<F> {
        let (k1, k2, k3) = aux;
        let PaxosSolverParams { l_size, r_size } = params;

        let i = hash2index(k1, x, l_size);
        let j = hash2index(k2, x, l_size);
        let l1 = p[i];
        let l2 = p[j];
        let vec_r = &p[l_size..];
        let inner_product = calc_r_inner_product(x, vec_r, k3, r_size);

        Ok(l1 + l2 + inner_product)
    }
}

// (boolean vector, F) tuple to construct constraints.
#[derive(Clone, Debug)]
pub(crate) struct ConstraintParts<F: FF> {
    v: Vec<bool>,
    f: F,
}

impl<F: FF> ConstraintParts<F> {
    pub(crate) fn new(v: Vec<bool>, f: F) -> Self {
        Self { v, f }
    }

    pub(crate) fn zero(r_size: usize) -> Self {
        Self {
            v: vec![false; r_size],
            f: F::zero(),
        }
    }

    pub(crate) fn add_other(&self, other: &Self) -> Self {
        let mut v = self.v.clone();
        for (i, val) in other.v.iter().enumerate() {
            v[i] ^= val;
        }
        let f = self.f + other.f;

        Self { v, f }
    }

    pub(crate) fn into(self) -> (Vec<bool>, F) {
        (self.v, self.f)
    }
}

pub(crate) use ConstraintParts as CP;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum VisitStatus {
    NotVisited,
    VisitedOnce,  // for find constraints
    VisitedTwice, // for calc vec_l
}

use VisitStatus::*;

#[derive(Debug)]
pub(crate) struct Edge<F: FF> {
    point: (F, F),
    visit_status: VisitStatus,
    back_edge: bool,
}

impl<F: FF> Edge<F> {
    pub(crate) fn is_visited(&self, mode: Mode) -> bool {
        match mode {
            FindConstraints => self.visit_status != NotVisited,
            CalcVecL => self.visit_status == VisitedTwice,
        }
    }
}

#[derive(Debug)]
pub(crate) struct DirectTo<F: FF> {
    to: Weak<RefCell<Node<F>>>,
    edge: Rc<RefCell<Edge<F>>>,
}

#[derive(Debug)]
pub(crate) struct Node<F: FF> {
    id: usize,
    dirs: Vec<DirectTo<F>>,
    visit_status: VisitStatus,
    accumulator: CP<F>,
}

#[derive(Clone, Copy)]
pub(crate) enum Mode {
    FindConstraints,
    CalcVecL,
}

use Mode::*;

impl<F: FF> Node<F> {
    pub(crate) fn is_visited(&self, mode: Mode) -> bool {
        match mode {
            FindConstraints => self.visit_status != NotVisited,
            CalcVecL => self.visit_status == VisitedTwice,
        }
    }

    pub(crate) fn next_dirs(
        &self,
        mode: Mode,
    ) -> Vec<(Rc<RefCell<Node<F>>>, Rc<RefCell<Edge<F>>>)> {
        self.dirs
            .iter()
            .filter_map(|dir| {
                let to = dir.to.upgrade().unwrap();
                let edge = Rc::clone(&dir.edge);

                if edge.borrow().is_visited(mode) || edge.borrow().back_edge {
                    None
                } else {
                    Some((to, edge))
                }
            })
            .collect::<Vec<_>>()
    }
}

fn construct_cuckoo_graph<F: FF>(
    points: &[(F, F)],
    keys: (u64, u64, u64),
    params: PaxosSolverParams,
) -> Vec<Rc<RefCell<Node<F>>>> {
    let (k1, k2, _) = keys;
    let m = params.l_size; // m = |L| = 2.01 * set.len()
    let r_size = params.r_size;
    let mut nodes: Vec<Option<Rc<RefCell<Node<F>>>>> = vec![None; m]; // random accessable table for nodes. using it for upsert_node.
    let mut result = Vec::with_capacity(m); // available nodes they will be included in the result.

    let mut upsert_node = |i: usize| match nodes[i].as_ref() {
        Some(node) => node.clone(),
        None => {
            let new_node = Rc::new(RefCell::new(Node {
                id: i,
                dirs: Vec::new(),
                visit_status: NotVisited,
                accumulator: CP::zero(r_size),
            }));
            nodes[i] = Some(new_node.clone());
            result.push(new_node.clone());
            new_node
        }
    };

    for &point in points {
        let x = point.0;
        let i = hash2index(k1, x, m);
        let j = hash2index(k2, x, m);

        let node_i = upsert_node(i);
        let node_j = upsert_node(j);

        let edge = Rc::new(RefCell::new(Edge {
            point,
            visit_status: NotVisited,
            back_edge: false,
        }));

        node_i.borrow_mut().dirs.push(DirectTo {
            to: Rc::downgrade(&node_j),
            edge: Rc::clone(&edge),
        });
        node_j.borrow_mut().dirs.push(DirectTo {
            to: Rc::downgrade(&node_i),
            edge,
        });
    }

    result
}

// #[derive(Clone, PartialEq, Eq)]
enum TofcRecRes<F: FF> {
    NoProblem,
    BackEdge(CP<F>),
}

fn dfs_to_find_constraints<F: FF>(
    graph: &[Rc<RefCell<Node<F>>>],
    keys: (u64, u64, u64),
    params: PaxosSolverParams,
) -> (Vec<(Vec<bool>, F)>, Vec<Rc<RefCell<Node<F>>>>) {
    let k3 = keys.2;
    let r_size = params.r_size;
    let mut constraints = Vec::new();
    let mut new_graph = Vec::with_capacity(graph.len());

    for node in graph.iter() {
        if node.borrow().is_visited(FindConstraints) {
            continue;
        }

        new_graph.push(Rc::clone(node));

        let total = CP::zero(r_size);
        let _ = dfs_tofc_rec(Rc::clone(node), total, k3, r_size, &mut constraints);
    }

    (constraints, new_graph)
}

use TofcRecRes::*;

/*
n1 -- e1 -> n2 -- e2 -> n3 -- e3 -> n4
                        ^           |
                        +---- e4 ---+

e4 is back_edge. then, proper constraints is e3.cp + e4.cp

to calc this, below function uses

n3.acc + (n4.acc + e4.cp)
= (e1.cp + e2.cp) + (e1.cp + e2.cp + e3.cp + e4.cp)
= e1.cp + e1.cp + e2.cp + e2.cp + e3.cp + e4.cp
= e3.cp + e4.cp

since same cps' xoring is 0.
*/

fn dfs_tofc_rec<F: FF>(
    node: Rc<RefCell<Node<F>>>,
    total: CP<F>,
    k3: u64,
    r_size: usize,
    result: &mut Vec<(Vec<bool>, F)>,
) -> TofcRecRes<F> {
    if node.borrow().is_visited(FindConstraints) {
        let cp = node.borrow().accumulator.clone();
        return BackEdge(cp);
    }

    {
        let mut n = node.borrow_mut();
        n.visit_status = VisitedOnce;
        n.accumulator = total.clone();
    }

    for (next_node, next_edge) in node.borrow().next_dirs(FindConstraints) {
        // follow the case of the edge is self loop
        if next_edge.borrow().is_visited(FindConstraints) {
            continue;
        }

        next_edge.borrow_mut().visit_status = VisitedOnce;

        let cp = {
            let x = next_edge.borrow().point.0;
            let v = r(k3, x, r_size);
            let f = next_edge.borrow().point.1;
            CP::new(v, f)
        };
        let next_total = total.add_other(&cp);
        let res = dfs_tofc_rec(next_node, next_total.clone(), k3, r_size, result);

        if let BackEdge(cp) = res {
            next_edge.borrow_mut().back_edge = true;

            let cp = next_total.add_other(&cp);
            result.push(cp.into());
        }
    }

    NoProblem
}

fn adjust_vec_r<F: FF>(equations: &[(usize, Vec<bool>, F)], vec_r: &mut [F]) {
    for (i, bits, val) in equations.iter() {
        let mut sum = val.clone();
        for (j, b) in bits.iter().enumerate() {
            if *i == j {
                continue;
            }

            if *b {
                sum += vec_r[j];
            }
        }
        vec_r[*i] = sum;
    }
}

fn dfs_to_calc_vec_l<F: FF>(
    graph: &[Rc<RefCell<Node<F>>>],
    keys: (u64, u64, u64),
    params: PaxosSolverParams,
    vec_r: &[F],
    vec_l: &mut [F],
) {
    let k3 = keys.2;
    let r_size = params.r_size;

    for node in graph.iter() {
        if node.borrow().is_visited(CalcVecL) {
            continue;
        }

        dfs_tocvl_rec(Rc::clone(node), k3, r_size, vec_r, vec_l);
    }
}

fn dfs_tocvl_rec<F: FF>(
    node: Rc<RefCell<Node<F>>>,
    k3: u64,
    r_size: usize,
    vec_r: &[F],
    vec_l: &mut [F],
) {
    if node.borrow().is_visited(CalcVecL) {
        panic!("Unreachable");
    }

    node.borrow_mut().visit_status = VisitedTwice;
    let u = node.borrow().id;

    for (next_node, next_edge) in node.borrow().next_dirs(CalcVecL) {
        // follow the case of the edge is self loop
        if next_edge.borrow().is_visited(CalcVecL) {
            continue;
        }

        next_edge.borrow_mut().visit_status = VisitedTwice;
        let v = next_node.borrow().id;

        let x = next_edge.borrow().point.0;
        let inner_product = calc_r_inner_product(x, vec_r, k3, r_size);
        let y = next_edge.borrow().point.1;

        vec_l[v] = vec_l[u] + inner_product + y;

        dfs_tocvl_rec(next_node, k3, r_size, vec_r, vec_l);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash_utils::hash_f;
    use rand::distributions::{Distribution, Standard};
    use rand::Rng;
    use scuttlebutt::field::{F128b, FiniteField};
    use scuttlebutt::AesRng;

    fn create_set<F: FiniteField>(set_size: usize) -> Vec<F>
    where
        Standard: Distribution<F>,
    {
        let mut rng = AesRng::new();

        let set = (0..set_size).map(|_| rng.gen()).collect::<Vec<_>>();

        set
    }

    fn test_paxos_base(set_size: usize, verbose: bool) {
        let set = create_set::<F128b>(set_size);

        let mut rng = AesRng::new();
        let aux = PaxosSolver::<F128b>::gen_aux(&mut rng).unwrap();
        let params = PaxosSolver::<F128b>::calc_params(set.len());

        let points = set
            .iter()
            .map(|x| (*x, hash_f(*x).unwrap()))
            .collect::<Vec<_>>();

        let p = PaxosSolver::encode(&mut rng, &points, aux, params).unwrap();

        if verbose {
            println!("p: {:?}", p);
        }

        let reconstructed_ys = set
            .iter()
            .map(|x| PaxosSolver::decode(&p, *x, aux, params).unwrap())
            .collect::<Vec<_>>();

        let ys = points.iter().map(|(_, y)| *y).collect::<Vec<_>>();

        assert_eq!(ys, reconstructed_ys);
    }

    #[test]
    fn test_paxos_small() {
        for n in 1..=10 {
            test_paxos_base(n, true);
        }
    }

    #[test]
    fn test_paxos_big() {
        for n in 10..100 {
            test_paxos_base(n, false);
        }

        for e in 10..21 {
            let n = 2usize.pow(e);
            test_paxos_base(n, false);
        }
    }

    #[test]
    fn test_paxos_2e20() {
        test_paxos_base(1 << 20, false);
    }
}

mod graphs_for_test;

#[cfg(test)]
mod detail_tests {
    use super::graphs_for_test::*;
    use super::*;
    use crate::set_utils::FromU128;
    use scuttlebutt::field::F128b;
    use scuttlebutt::AesRng;
    // use rand::Rng;

    fn test_construct_cuckoo_graph_base(n: usize, verbose: bool) {
        let mut rng = AesRng::new();
        let aux = PaxosSolver::<F128b>::gen_aux(&mut rng).unwrap();
        let params = PaxosSolver::<F128b>::calc_params(n);

        let points = (0..n)
            .map(|i| (F128b::from_u128(i as _), F128b::from_u128(i as _)))
            .collect::<Vec<(F128b, F128b)>>();

        let graph = construct_cuckoo_graph(&points, aux, params);

        let (analyzed_result, _new_graph) = graph_analyze(&graph, verbose, verbose);

        println!("analyzed_result:\n{}", analyzed_result);
    }

    #[test]
    fn test_construct_cuckoo_graph_small() {
        for n in 0..=10 {
            test_construct_cuckoo_graph_base(n, true);
        }
    }

    #[test]
    fn test_construct_cuckoo_graph_big() {
        for n in 10..100 {
            test_construct_cuckoo_graph_base(n, false);
        }

        for e in 10..21 {
            let n = 2usize.pow(e);
            test_construct_cuckoo_graph_base(n, false);
        }
    }

    #[test]
    fn test_construct_cuckoo_graph_2e20() {
        test_construct_cuckoo_graph_base(1 << 20, false);
    }

    #[test]
    fn test_find_constraints_for_fixnum_graphs() {
        let mut rng = AesRng::new();
        let aux = PaxosSolver::<F128b>::gen_aux(&mut rng).unwrap();
        let params = PaxosSolver::<F128b>::calc_params(20);

        let create_funcs: Vec<fn(PaxosSolverParams) -> Vec<Rc<RefCell<Node<F128b>>>>> = vec![
            create_specific_graph_empty,
            create_big_dipper_graph,
            create_triangle_graph,
            create_double_constraints_graph_0,
            create_double_constraints_graph_1,
            create_multi_constraints_graph,
            create_bird_graph,
        ];

        let verbose = true;

        for func in create_funcs.into_iter() {
            println!("\n####################################\n");

            let graph = func(params);

            let (analyzed_result, _new_graph) = graph_analyze(&graph, verbose, verbose);

            println!("analyzed_result:\n{}", analyzed_result);

            let graph = func(params);

            let (constraints, _new_graph) = dfs_to_find_constraints(&graph, aux, params);

            println!("constraints ({}):", constraints.len());

            for cons in constraints.into_iter() {
                println!("{:?}", cons);
            }
        }
    }

    fn test_find_constraints_for_specific_graphs_base(n: usize, verbose: bool) {
        let mut rng = AesRng::new();
        let aux = PaxosSolver::<F128b>::gen_aux(&mut rng).unwrap();
        let params = PaxosSolver::<F128b>::calc_params(2 * n);

        let create_funcs: Vec<fn(PaxosSolverParams, usize) -> Vec<Rc<RefCell<Node<F128b>>>>> = vec![
            create_specific_graph_no_edge,
            create_specific_graph_self_loop,
            create_specific_graph_straight,
            create_specific_graph_discrete_straight,
            create_specific_graph_straight_with_self_loop,
            create_specific_graph_straight_with_back_edges,
        ];

        for func in create_funcs.into_iter() {
            println!("\n####################################\n");

            let graph = func(params, n);

            let (analyzed_result, _new_graph) = graph_analyze(&graph, verbose, verbose);

            println!("analyzed_result:\n{}", analyzed_result);

            let graph = func(params, n);

            let (constraints, _new_graph) = dfs_to_find_constraints(&graph, aux, params);

            println!("constraints ({}):", constraints.len());

            if verbose {
                for cons in constraints.into_iter() {
                    println!("{:?}", cons);
                }
            }
        }
    }

    #[test]
    fn test_find_constraints_for_specific_graphs_small() {
        for n in 0..=10 {
            println!(
                "\n%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%% {} %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%\n",
                n
            );
            test_find_constraints_for_specific_graphs_base(n, true);
        }
    }

    /* // stack overflow. the overflow probability would be small in random case. so we don't need to test this.
    #[test]
    fn test_find_constraints_for_specific_graphs_big() {
        for e in 10..=20 {
            println!(
                "\n%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%% 2^{} %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%\n",
                e
            );
            test_find_constraints_for_specific_graphs_base(1 << e, false);
        }
    }
    */

    fn test_find_constraints_base(n: usize, verbose: bool) {
        let mut rng = AesRng::new();
        let aux = PaxosSolver::<F128b>::gen_aux(&mut rng).unwrap();
        let params = PaxosSolver::<F128b>::calc_params(n);

        let points = (0..n)
            .map(|i| (F128b::from_u128(i as _), F128b::from_u128(i as _)))
            .collect::<Vec<(F128b, F128b)>>();

        let graph = construct_cuckoo_graph(&points, aux, params);

        let (constraints, _new_graph) = dfs_to_find_constraints(&graph, aux, params);

        println!("constraints ({}):", constraints.len());

        if verbose {
            for cons in constraints.into_iter() {
                println!("{:?}", cons);
            }
        }
    }

    #[test]
    fn test_find_constraints_small() {
        for n in 0..=10 {
            test_find_constraints_base(n, true);
        }
    }

    #[test]
    fn test_find_constraints_big() {
        for n in 10..100 {
            test_find_constraints_base(n, false);
        }

        for e in 10..21 {
            let n = 2usize.pow(e);
            test_find_constraints_base(n, false);
        }
    }

    #[test]
    fn test_find_constraints_2e20() {
        test_find_constraints_base(1 << 20, false);
    }

    #[test]
    fn test_dfs_to_calc_vec_l_for_fixnum_graphs() {
        let mut rng = AesRng::new();
        let aux = PaxosSolver::<F128b>::gen_aux(&mut rng).unwrap();
        let params = PaxosSolver::<F128b>::calc_params(20);

        let create_funcs: Vec<fn(PaxosSolverParams) -> Vec<Rc<RefCell<Node<F128b>>>>> = vec![
            create_specific_graph_empty,
            create_big_dipper_graph,
            create_triangle_graph,
            create_double_constraints_graph_0,
            create_double_constraints_graph_1,
            create_multi_constraints_graph,
            create_bird_graph,
        ];

        let verbose = true;

        for func in create_funcs.into_iter() {
            println!("\n####################################\n");

            // 2. Initialize variables $L$ and $R$ and an initialliy empty set of linear constraints S.
            let mut vec_l: Vec<F128b> = (0..params.l_size).map(|_| rng.gen()).collect::<Vec<_>>();
            let mut vec_r: Vec<F128b> = (0..params.r_size).map(|_| rng.gen()).collect::<Vec<_>>();

            let graph = func(params);

            let (analyzed_result, _new_graph) = graph_analyze(&graph, verbose, verbose);

            println!("analyzed_result:\n{}", analyzed_result);

            let graph = func(params);

            let (constraints, _new_graph) = dfs_to_find_constraints(&graph, aux, params);

            println!("constraints ({}):", constraints.len());

            for cons in constraints.iter() {
                println!("{:?}", cons);
            }

            if constraints.len() > params.r_size {
                panic!("too many constraints");
            }

            if constraints.len() > 0 {
                // 4. Solve for variables R satisfying the constraints of system S.
                let equations_w = match gaussian_elimination(constraints) {
                    Ok(equs) => equs,
                    Err(e) => {
                        println!("gaussian elimination Error: {:?}", e);
                        continue;
                    }
                };
                let Some(equations) = equations_w else {
                    println!("failed to solve linear equations");
                    continue;
                };

                // 4'. Adjust vec_r by equations.
                adjust_vec_r(&equations, &mut vec_r);
            } else {
                println!("no constraints");
            }

            // 5. Perform another DFS on $G_{h_1, h_2, X}$ to compute $L$.
            dfs_to_calc_vec_l(&graph, aux, params, &vec_r, &mut vec_l);

            // 6. Output D = L || R
            let mut result = vec_l;
            result.extend(vec_r);

            println!("result (len: {}): {:?}", result.len(), result);
        }
    }

    fn test_dfs_to_calc_vec_l_for_specific_graphs_base(n: usize, verbose: bool) {
        let mut rng = AesRng::new();
        let aux = PaxosSolver::<F128b>::gen_aux(&mut rng).unwrap();
        let params = PaxosSolver::<F128b>::calc_params(2 * n);

        let create_funcs: Vec<fn(PaxosSolverParams, usize) -> Vec<Rc<RefCell<Node<F128b>>>>> = vec![
            create_specific_graph_no_edge,
            create_specific_graph_self_loop,
            create_specific_graph_straight,
            create_specific_graph_discrete_straight,
            create_specific_graph_straight_with_self_loop,
            create_specific_graph_straight_with_back_edges,
        ];

        for func in create_funcs.into_iter() {
            println!("\n####################################\n");

            // 2. Initialize variables $L$ and $R$ and an initialliy empty set of linear constraints S.
            let mut vec_l: Vec<F128b> = (0..params.l_size).map(|_| rng.gen()).collect::<Vec<_>>();
            let mut vec_r: Vec<F128b> = (0..params.r_size).map(|_| rng.gen()).collect::<Vec<_>>();

            let graph = func(params, n);

            let (analyzed_result, _new_graph) = graph_analyze(&graph, verbose, verbose);

            println!("analyzed_result:\n{}", analyzed_result);

            let graph = func(params, n);

            let (constraints, _new_graph) = dfs_to_find_constraints(&graph, aux, params);

            println!("constraints ({}):", constraints.len());

            if verbose {
                for cons in constraints.iter() {
                    println!("{:?}", cons);
                }
            }

            if constraints.len() > params.r_size {
                panic!("too many constraints");
            }

            if constraints.len() > 0 {
                // 4. Solve for variables R satisfying the constraints of system S.
                let equations_w = match gaussian_elimination(constraints) {
                    Ok(equs) => equs,
                    Err(e) => {
                        println!("gaussian elimination Error: {:?}", e);
                        continue;
                    }
                };
                let Some(equations) = equations_w else {
                    println!("failed to solve linear equations");
                    continue;
                };

                // 4'. Adjust vec_r by equations.
                adjust_vec_r(&equations, &mut vec_r);
            } else {
                println!("no constraints");
            }

            // 5. Perform another DFS on $G_{h_1, h_2, X}$ to compute $L$.
            dfs_to_calc_vec_l(&graph, aux, params, &vec_r, &mut vec_l);

            // 6. Output D = L || R
            let mut result = vec_l;
            result.extend(vec_r);

            if verbose {
                println!("result (len: {}): {:?}", result.len(), result);
            }
        }
    }

    #[test]
    fn test_dfs_to_calc_vec_l_for_specific_graphs_small() {
        for n in 0..=10 {
            println!(
                "\n%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%% {} %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%\n",
                n
            );
            test_dfs_to_calc_vec_l_for_specific_graphs_base(n, true);
        }
    }

    fn test_dfs_to_calc_vec_l_base(n: usize, verbose: bool) {
        let mut rng = AesRng::new();
        let aux = PaxosSolver::<F128b>::gen_aux(&mut rng).unwrap();
        let params = PaxosSolver::<F128b>::calc_params(n);

        // 2. Initialize variables $L$ and $R$ and an initialliy empty set of linear constraints S.
        let mut vec_l: Vec<F128b> = (0..params.l_size).map(|_| rng.gen()).collect::<Vec<_>>();
        let mut vec_r: Vec<F128b> = (0..params.r_size).map(|_| rng.gen()).collect::<Vec<_>>();

        let points = (0..n)
            .map(|i| (F128b::from_u128(i as _), F128b::from_u128(i as _)))
            .collect::<Vec<(F128b, F128b)>>();

        let graph = construct_cuckoo_graph(&points, aux, params);

        let (constraints, _new_graph) = dfs_to_find_constraints(&graph, aux, params);

        println!("constraints ({}):", constraints.len());

        if verbose {
            for cons in constraints.iter() {
                println!("{:?}", cons);
            }
        }

        if constraints.len() > params.r_size {
            panic!("too many constraints");
        }

        if constraints.len() > 0 {
            // 4. Solve for variables R satisfying the constraints of system S.
            let equations_w = gaussian_elimination(constraints).unwrap();
            let Some(equations) = equations_w else {
                panic!("failed to solve linear equations");
            };

            // 4'. Adjust vec_r by equations.
            adjust_vec_r(&equations, &mut vec_r);
        } else {
            println!("no constraints");
        }

        // 5. Perform another DFS on $G_{h_1, h_2, X}$ to compute $L$.
        dfs_to_calc_vec_l(&graph, aux, params, &vec_r, &mut vec_l);

        // 6. Output D = L || R
        let mut result = vec_l;
        result.extend(vec_r);

        if verbose {
            println!("result (len: {}): {:?}", result.len(), result);
        }

        for (x, y) in points.iter() {
            let reconstructed_y = PaxosSolver::decode(&result, *x, aux, params).unwrap();

            assert_eq!(*y, reconstructed_y);
        }
    }

    #[test]
    fn test_dfs_to_calc_vec_l_small() {
        for n in 1..=10 {
            test_dfs_to_calc_vec_l_base(n, true);
        }
    }

    #[test]
    fn test_dfs_to_calc_vec_l_big() {
        for n in 10..100 {
            test_dfs_to_calc_vec_l_base(n, false);
        }

        for e in 10..21 {
            let n = 2usize.pow(e);
            test_dfs_to_calc_vec_l_base(n, false);
        }
    }

    #[test]
    fn test_dfs_to_calc_vec_l_2e20() {
        test_dfs_to_calc_vec_l_base(1 << 20, false);
    }
}
