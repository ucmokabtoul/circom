use std::ops::Deref;
use num_traits::FromPrimitive;
use program_structure::program_archive::ProgramArchive;
use program_structure::ast::{Statement, Expression, Meta, ExpressionInfixOpcode};
use super::lint::LintLevel;
use super::{build_statement_flowgraph, assignments_to_x};
use super::lint::Lint;
use num_bigint::BigInt;
use program_structure::error_code::ReportCode;
use program_structure::file_definition;
use petgraph::graph::{NodeIndex, node_index};
use petgraph::algo;

/// Implements the TerminationAnalyser
pub struct TerminationAnalyser {
    for_loop: Option<ForLoop>,
}

pub struct ForLoop {
    counter: String,
    init: BigInt,
    bound: BigInt,
    pub cond: Expression,
    loop_body: Statement,
    program: ProgramArchive,
}

impl ForLoop {

    fn new(program: &ProgramArchive, init_block: &Statement, cond: &Expression, body: &Statement) -> Option<Self> {
        let init: BigInt;
        let bound: BigInt;
        //TODO: improve this pattern matching/switching. can it be abstracted into a parser
        // function?
        if let Statement::InitializationBlock { initializations, .. } = init_block {
            //FIXME: array access, check is not empty
            if let Some(Statement::Declaration { .. }) = &initializations.first() {
                if let Statement::Substitution { var, rhe, ..} = &initializations[1] {
                    let counter = var.to_owned();
                    if let Expression::Number(_meta, big_int) = rhe {
                        //FIXME: biging
                        init = big_int.clone();
                        // Condition is of the form:
                        // "< Number"
                        // TODO: add LesserEq
                        if let Expression::InfixOp { lhe, infix_op, rhe, .. } = cond {
                            if *infix_op == ExpressionInfixOpcode::Lesser {
                                match lhe.deref() {
                                    Expression::Variable { name, .. } => {
                                        // left-hand side of condition is the counter variable.
                                        if *name == counter {
                                            if let Expression::Number(_meta, big_int) = rhe.deref() {
                                                bound = big_int.clone();

                                                return Some(ForLoop { counter: counter, init: init, bound: bound, cond: cond.clone(), loop_body: body.clone(), program: program.clone() })
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

use std::fmt;
impl fmt::Display for ForLoop {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "counter: {} init: {} cond: {} bound: {}", self.counter, self.init, meta_to_string(&self.program, &self.cond.get_meta()), self.bound)
    }
}

pub enum Monotonicity {
    Increasing,
    Decreasing,
    Constant,
    NotMonotonic,
}

pub fn is_for_loop(program: &ProgramArchive, statements: &Vec<Statement>) -> Option<ForLoop> {
    if statements.len() == 2 {
        let init = statements.first().unwrap();
        if let Statement::While { cond, stmt, .. } = statements.get(1).unwrap() {
            let body = stmt;
            if let Statement::Block { stmts, .. } = body.deref() {
                if let Some(_step) = stmts.last() {
                    if let Some(for_loop) = ForLoop::new(program, init, cond, body) {
                        return Some(for_loop)
                    }
                }
            }
        }
    }
    None
}

impl TerminationAnalyser {
    pub fn new(for_loop: Option<ForLoop>) -> TerminationAnalyser {
        TerminationAnalyser{for_loop}
    }

    pub fn analyse(&mut self, program: &ProgramArchive) -> Vec<Lint> {
        let mut lints = Vec::new();
        let mut n_paths= 0;
        if let Some(for_loop) = &self.for_loop {
            //TODO: lint should be at the exact step;
            let graph = build_statement_flowgraph(program, &for_loop.loop_body);
            let start = node_index(0);
            let end = node_index(graph.node_count()-1);
            //FIXME: maybe end has to be statement after loop (or introduce explicit END node)
            let paths= algo::all_simple_paths::<Vec<_>, _>(&graph, start, end, 0, None)
                .collect::<Vec<_>>();
            n_paths = paths.len();
            let mut incrs = vec![];
            for path in paths.iter() {
                let steps = assignments_to_x(&graph, &path, &for_loop.counter);
                match self.analyse_termination(&for_loop, &steps) {
                    (Some(lint), _incr) =>{
                        lints.push(lint);
                    }
                    (None, incr) =>{
                        incrs.push(incr.unwrap().clone());
                    }
                }
            }
            //NOTE (@okabtoul): Used for studing resource analysis
            //_analyse_resource_usage(&paths, &incrs);
        }
        // if all paths are bad, it's an error.
        // Otherwise it's only a warning
        if n_paths == lints.len() {
            for lint in lints.iter_mut() {
                lint.level = LintLevel::Error
            }
        }
        lints
    }


    // Returns a BigInt representing the increment in each step of the loop.
    // None if increments are not constant.
    fn eval_step(&self, counter: &str, step: &Statement, _program: &ProgramArchive) -> Option<BigInt> {
        if let Statement::Substitution { rhe, .. } = step {
            match rhe {
                Expression::InfixOp { lhe, infix_op, rhe, .. } => {
                    match lhe.deref() {
                        Expression::Variable { name, .. } => {
                            if *name == counter {
                                if let Expression::Number(_meta, big_int) = rhe.deref() {
                                    match infix_op {
                                        ExpressionInfixOpcode::Add => { return Some(big_int.clone()); }
                                        ExpressionInfixOpcode::Sub => { return Some(-big_int.clone()); }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn analyse_monotonicty(&self, _init: &BigInt, _bound: &BigInt, incr: &BigInt) -> Monotonicity {
        let delta = incr;
        if *delta > BigInt::from_u8(0).unwrap() {
            Monotonicity::Decreasing
        } else if *delta < BigInt::from_u8(0).unwrap() {
            Monotonicity::Increasing
        } else {
            Monotonicity::Constant
        }
    }

    pub fn analyse_termination(&self, for_loop: &ForLoop, steps: &[Statement]) -> (Option<Lint>, Option<BigInt>) {
        let incr: Option<BigInt> = steps.iter().map(|s| self.eval_step(&for_loop.counter, s, &for_loop.program)).filter(|n| n.is_some()).sum();
        let monotonicity = match &incr {
            Some(incr) => {
                self.analyse_monotonicty(&for_loop.init, &for_loop.bound, &incr)
            }
            None => Monotonicity::NotMonotonic
        };
        match monotonicity {
            Monotonicity::Constant => {
                (Some(Lint {
                    error_code: ReportCode::LoopNoProgress,
                    error_msg: format!("Loop does not progress"),
                    loc: file_definition::generate_file_location(
                        steps.first().unwrap().get_meta().get_start(),
                        steps.first().unwrap().get_meta().get_end(),
                    ),

                    msg: format!(
                        "",
                    ),
                    level: LintLevel::Warning,

                }), incr)
            },
            Monotonicity::Increasing => {
                (Some(Lint {
                    error_code: ReportCode::LoopMayOverflow,
                    error_msg: format!("Loop may overflow: refer to Circom's docs on modular field arithmetic: https://docs.circom.io/circom-language/basic-operators/#field-elements"),
                    loc: file_definition::generate_file_location(
                        steps.first().unwrap().get_meta().get_start(),
                        steps.first().unwrap().get_meta().get_end(),
                    ),

                    msg: format!(
                        "",
                    ),
                    level: LintLevel::Warning,
                }), incr)
            },
            Monotonicity::Decreasing => { (None, incr) }
            _ => { (None, None) }
        }
    }

}

/// print_expr returns the expression e as it literally appeared in program.
fn meta_to_string(program: &ProgramArchive, m: &Meta) -> String {
    let sources = program.get_file_library().to_storage();
    let source = sources.get(m.get_file_id()).unwrap();
    let s = source.source().as_str();
    let loc = m.location.clone();
    s[loc].to_string()
}

fn _analyse_resource_usage(_paths: &[Vec<NodeIndex>], incrs: &[BigInt]) {
    if let Some(min) = incrs.iter().min() {
        println!("min incr: {}", min);
    }
}
