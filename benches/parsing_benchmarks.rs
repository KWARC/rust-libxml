#[macro_use]
extern crate criterion;

use criterion::Criterion;
use libxml::parser::Parser;
use libxml::tree::{Node, NodeType};
use rayon::prelude::*;

// -- workhorse functions
// not *quite* classic depth-first search, since we keep all children at the current level in memory,
// but certainly DFS-order for traversal

fn dfs_single(node: Node) -> i32 {
  1 + node
    .get_child_nodes()
    .into_iter()
    .map(dfs_single)
    .sum::<i32>()
}

fn dfs_parallel(node: Node) -> i32 {
  1 + node
    .get_child_nodes()
    .into_par_iter()
    .map(dfs_parallel)
    .sum::<i32>()
}

fn dfs_single_work2(node: Node) -> (i32, usize) {
  if node.get_type() == Some(NodeType::TextNode) {
    (1, node.get_content().len())
  } else {
    node
      .get_child_nodes()
      .into_iter()
      .map(dfs_single_work2)
      .fold((1, 0), |acc, x| (acc.0 + x.0, acc.1 + x.1))
  }
}

fn dfs_parallel_work2(node: Node) -> (i32, usize) {
  if node.get_type() == Some(NodeType::TextNode) {
    (1, node.get_content().len())
  } else {
    let dfs_work = node
      .get_child_nodes()
      .into_par_iter()
      .map(dfs_parallel_work2)
      .reduce(|| (0, 0), |acc, x| (acc.0 + x.0, acc.1 + x.1));
    (dfs_work.0 + 1, dfs_work.1)
  }
}

// --- bencher functions

fn bench_single_thread(c: &mut Criterion) {
  let parser = Parser::default();
  let doc = parser.parse_file("benches/big.xml").unwrap();
  c.bench_function("single thread DFS count", move |b| {
    b.iter(|| {
      let root = doc.get_root_element().unwrap();
      assert_eq!(dfs_single(root), 4_690_647)
    })
  });
}

fn bench_single_thread_work2(c: &mut Criterion) {
  let parser = Parser::default();
  let doc = parser.parse_file("benches/big.xml").unwrap();
  c.bench_function("single thread DFS count+length", move |b| {
    b.iter(|| {
      let root = doc.get_root_element().unwrap();
      assert_eq!(dfs_single_work2(root), (4_690_647, 81_286_567))
    })
  });
}

fn bench_multi_thread(c: &mut Criterion) {
  let parser = Parser::default();
  let doc = parser.parse_file("benches/big.xml").unwrap();
  c.bench_function("multi thread DFS count", move |b| {
    b.iter(|| {
      let root = doc.get_root_element().unwrap();
      assert_eq!(dfs_parallel(root), 4_690_647);
    })
  });
}

fn bench_multi_thread_work2(c: &mut Criterion) {
  let parser = Parser::default();
  let doc = parser.parse_file("benches/big.xml").unwrap();
  c.bench_function("multi thread DFS count+length", move |b| {
    b.iter(|| {
      let root = doc.get_root_element().unwrap();
      assert_eq!(dfs_parallel_work2(root), (4_690_647, 81_286_567))
    })
  });
}
criterion_group!(
  name = benches;
  config = Criterion::default().sample_size(10);
  targets = bench_single_thread, bench_single_thread_work2, bench_multi_thread, bench_multi_thread_work2
);

criterion_main!(benches);

// -- Results on @dginev's machine, big.xml
//    controlling thread count via "RAYON_NUM_THREADS=X cargo bench"
// ---
// 32 threads:
// ---
// multi thread DFS count  time:   [4.6973 s 4.7191 s 4.7351 s]
// multi thread DFS count+length
//                         time:   [4.6695 s 4.6784 s 4.6864 s]
// ---
// 16 threads (compared to 32 threads):
// ---
// multi thread DFS count  time:   [4.3077 s 4.3226 s 4.3503 s]
//                         change: [-9.2711% -8.5028% -7.6743%] (p = 0.00 < 0.05)
// multi thread DFS count+length
//                         time:   [4.3969 s 4.4199 s 4.4400 s]
//                         change: [-6.2258% -5.7250% -5.2269%] (p = 0.00 < 0.05)
// ---
// 8 threads (compared to 16 threads):
// ---
// multi thread DFS count  time:   [4.6934 s 4.7066 s 4.7236 s]
//                         change: [+8.0596% +8.9386% +9.6914%] (p = 0.00 < 0.05)
// multi thread DFS count+length
//                         time:   [4.2944 s 4.3431 s 4.3800 s]
//                         change: [-2.9167% -1.8463% -0.7335%] (p = 0.00 < 0.05)
// ---
// 4 threads (compared to 8 threads):
// ---
// multi thread DFS count  time:   [3.9458 s 4.0396 s 4.1157 s]
//                         change: [-17.660% -15.890% -14.159%] (p = 0.00 < 0.05)
// multi thread DFS count+length
//                         time:   [3.8821 s 3.9815 s 4.0516 s]
//                         change: [-9.2847% -7.1400% -5.1074%] (p = 0.00 < 0.05)
// ---
// 2 threads (compared to 4 threads):
// ---
// multi thread DFS count  time:   [3.1924 s 3.2423 s 3.2986 s]
//                         change: [-20.691% -18.659% -16.551%] (p = 0.00 < 0.05)
// multi thread DFS count+length
//                         time:   [3.2541 s 3.4244 s 3.4956 s]
//                         change: [-24.243% -19.735% -15.510%] (p = 0.00 < 0.05)
// ---
// 1 thread (compared to 2 threads):
// ---
// multi thread DFS count  time:   [1.5219 s 1.5240 s 1.5262 s]
//                         change: [-53.428% -52.788% -52.130%] (p = 0.00 < 0.05)
// multi thread DFS count+length
//                         time:   [1.7658 s 1.7708 s 1.7761 s]
//                         change: [-47.766% -45.000% -41.973%] (p = 0.00 < 0.05)
// ---
// single thread DFS count time:   [1.4969 s 1.4997 s 1.5049 s]
// single thread DFS count+length
//                         time:   [1.7236 s 1.7319 s 1.7404 s]
