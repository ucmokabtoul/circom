#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use type_analysis::analyzers::cfg::build_flowgraph;
    use petgraph::dot::Dot;

    struct TestCFG<'a> {
        input: (&'a str, &'a str),
        expected: &'a str,
    }

    #[test]
    fn flowgraph() {
        let cases = vec![
            TestCFG {
                input: ("tests/fixtures/reassign-signal.circom", "A"),
                expected: "tests/fixtures/reassign-signal.dot",
                }];
        for case in cases {
            let mut in_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            in_path.push(case.input.0);
            let (program_archive, _) = parser::run_parser(
                in_path.to_str().unwrap().to_owned(),
                env!("CARGO_PKG_VERSION"),
                vec![in_path],
            )
            .ok()
            .unwrap();
            let template_data = program_archive.get_template_data(case.input.1);
            let graph = build_flowgraph(&program_archive, &template_data);
            assert_eq!(format!("{}", Dot::new(&graph)), fs::read_to_string(case.expected).unwrap());
        }
    }
}
