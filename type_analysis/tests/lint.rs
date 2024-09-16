#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use type_analysis::analyzers::lint::{
        Lint, StaticLinter, AnonComponentLinter, ConstantSignalLinter,
    };
    use type_analysis::check_types::check_types;
    use program_structure::error_code::ReportCode;
    use program_structure::file_definition::generate_file_location;

    struct TestLint<'a> {
        input: &'a str,
        expected: Vec<Lint>,
    }

    #[test]
    fn static_linter() {
        let cases = vec![
            TestLint {
                input: "tests/fixtures/hint-anon-component.circom",
                expected: vec![Lint {
                    error_code: ReportCode::AnonymousCompLint,
                    error_msg: format!("Anonymous component: `a`"),
                    loc: generate_file_location(234, 251),
                    msg: format!("You can use (_, salida) <== A()(in[0], in[1]);"),
                }],
            },
            TestLint {
                input: "tests/fixtures/intermediate-unknown-signal.circom",
                expected: vec![
                    Lint {
                        error_code: ReportCode::ConstantSignalLint,
                        error_msg: format!("Constant signal: `aux`"),
                        loc: generate_file_location(70, 87),
                        msg: format!(
                            "You should define `aux` as a variable instead: `var aux = 42`"
                        ),
                    },
                ],
            },
        ];
        for case in cases {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push(case.input);
            let (mut program_archive, _) = parser::run_parser(
                path.to_str().unwrap().to_owned(),
                env!("CARGO_PKG_VERSION"),
                vec![path],
            )
            .ok()
            .unwrap();
            let _ = check_types(&mut program_archive);
            let anon = AnonComponentLinter::new();
            let cons = ConstantSignalLinter::new();
            let mut analyser =
                StaticLinter::new(program_archive, vec![Box::new(anon), Box::new(cons)]);
            let lints = analyser.lint();
            assert!(lints == case.expected);
        }
    }
}
