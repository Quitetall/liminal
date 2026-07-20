//! Deterministic M17 report renderer.

use std::env;

fn main() {
    let mut args = env::args().skip(1);
    let git_commit = args
        .next()
        .expect("usage: report <40-hex measurement commit> <40-hex measurement tree>");
    let measurement_tree = args
        .next()
        .expect("usage: report <40-hex measurement commit> <40-hex measurement tree>");
    assert!(
        args.next().is_none(),
        "report accepts exactly two arguments"
    );

    let measurement = spike_real_cst::run_frozen_measurement()
        .expect("frozen real-CST measurement must complete");
    print!(
        "{}",
        spike_real_cst::render_report(&measurement, &git_commit, &measurement_tree)
    );
}
