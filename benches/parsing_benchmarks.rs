#[macro_use]
extern crate criterion;

use criterion::Criterion;
use libxml::parser::Parser;
use libxml::tree::Node;
use rayon::prelude::*;

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

fn bench_single_thread(c: &mut Criterion) {
  let parser = Parser::default();
  let doc = parser.parse_file("benches/big.xml").unwrap();
  c.bench_function("single thread", move |b| {
    b.iter(|| {
      let root = doc.get_root_element().unwrap();
      assert_eq!(dfs_single(root), 4_690_647)
    })
  });
}

fn bench_multi_thread(c: &mut Criterion) {
  let parser = Parser::default();
  let doc = parser.parse_file("benches/big.xml").unwrap();
  c.bench_function("multi thread", move |b| {
    b.iter(|| {
      let root = doc.get_root_element().unwrap();
      assert_eq!(dfs_parallel(root), 4_690_647);
    })
  });
}

criterion_group!(
  name = benches;
  config = Criterion::default().sample_size(10);
  targets = bench_single_thread, bench_multi_thread
);
criterion_main!(benches);
