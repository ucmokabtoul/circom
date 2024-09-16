use program_structure::ast::{Statement, Meta, VariableType, SignalType};
use program_structure::file_definition::{self, FileLocation};
use petgraph::dot::Dot;
use program_structure::program_archive::ProgramArchive;
use petgraph::graph::{Graph, NodeIndex, node_index};
use std::{collections::HashMap, collections::HashSet};

use petgraph::algo;



// On any given path, a signal/var should be assigned exactly once.
// An execution path that doesn't assign a value to every signal/var has a
// `Unassigned` error, while a double (or more) assignment is a
// `MultipleAssignment` error.
#[derive(PartialEq, Debug, Clone)]
enum PathAnalysisErr {
    Unassigned,
    MultipleAssignment,
}


pub struct PathAnalysisLint {
    _loc: FileLocation,
    _xtype: VariableType,
    _meta: Meta,
    _path_err: PathAnalysisErr,
    pub name: String,
    pub msg: String,
}

pub fn lint_all_paths_to_signal(program: &ProgramArchive, template_name: &str, signal: &str, meta: &Meta) -> Option<PathAnalysisLint> {
    let graph = build_flowgraph(&program, template_name);
    let root= node_index(0);
    // The meta of the node to check could be a call, an expression, not necessarily explicitly
    // represented in the CFG (but its enclosing node would).
    // So we need to find the node with a meta [start, end) spanning `meta`'s elem_id.
    let last= graph.node_indices().find(|&n| graph.node_weight(n).unwrap().stmt.get_meta().start <= meta.start && graph.node_weight(n).unwrap().stmt.get_meta().end >= meta.end).unwrap();

    let paths= algo::all_simple_paths::<Vec<_>, _>(&graph, root, last, 0, None)
        .collect::<Vec<_>>();

    for path in paths.iter() {
        let assignments = run_path_analysis(&graph, path);
        //FIXME: could the signal not have been declared?
        let (assignment, v) = assignments.get(signal).unwrap();
        if v.is_err() {
            match assignment.xtype {
                VariableType::Signal(SignalType::Input, _) => {},
                VariableType::Signal(SignalType::Intermediate, _)|VariableType::Signal(SignalType::Output, _) => {
                    let lint = match v {
                        Err(PathAnalysisErr::Unassigned) => PathAnalysisLint{_loc: meta.location.clone(), _meta: meta.clone(), _xtype: assignment.xtype.clone(), _path_err: PathAnalysisErr::Unassigned, name: signal.to_owned(),
                        msg: format!("Signal `{}` is not initialized on all execution paths and would be given a zero value where no assignment has been made to it. Consider assigning a value to it explicitly.", signal)},
                        Err(PathAnalysisErr::MultipleAssignment) => PathAnalysisLint{_loc: assignment.loc.clone(), _meta: meta.clone(), _xtype: assignment.xtype.clone(), _path_err: PathAnalysisErr::MultipleAssignment, name: signal.to_owned(), msg: format!("Signal {} was already assigned a value and multiple assignments are not allowed.", signal.to_owned())},
                        _ => { unreachable!("");}
                    };
                    return Some(lint);
                },
                VariableType::Var => {
                    let lint = match v {
                        Err(PathAnalysisErr::Unassigned) => PathAnalysisLint{_loc: assignment.loc.clone(), _meta: meta.clone(), _xtype: assignment.xtype.clone(), _path_err: PathAnalysisErr::Unassigned, name: signal.to_owned(),
                        msg: format!("Var `{}` is not initialized on all execution paths and would be given a zero value where no assignment has been made to it. Consider assigning a value to it explicitly.", signal)},
                        Err(PathAnalysisErr::MultipleAssignment) => PathAnalysisLint{_loc: assignment.loc.clone(), _meta: meta.clone(), _xtype: assignment.xtype.clone(), _path_err: PathAnalysisErr::MultipleAssignment, name: signal.to_owned(), msg: format!("Signal {} is not initialized on this path and would be given a zero value. Consider assigning a value to it explicitly", signal.to_owned())},
                        _ => { unreachable!("");}
                    };
                    return Some(lint);
                },
                //FIXME: any other relevant types?
                _ => {},
            }
        }
    }
    None
}


#[derive(PartialEq, Clone)]
struct AssignmentVal {
    xtype: VariableType,
    loc: FileLocation,
}

type Assignments = HashMap<String, (AssignmentVal, Result<(), PathAnalysisErr>)>;

fn run_path_analysis(graph: &CFG, path: &[NodeIndex]) -> Assignments {
    let mut assignments = Assignments::new();
    for node in path.iter() {
        match graph.node_weight(*node).unwrap().stmt {
            Statement::Declaration { name, xtype, meta, ..} => {
                assignments.insert(name.to_owned(), (AssignmentVal{xtype: xtype.clone(), loc: file_definition::generate_file_location(
                            meta.get_start(),
                            meta.get_end(),
                )},  Err(PathAnalysisErr::Unassigned)));
            }
            Statement::Substitution { var, .. } => {
                let declared_name = assignments.get_mut(var);
                match declared_name {
                    Some((assignment, Err(PathAnalysisErr::Unassigned))) => {
                        *declared_name.unwrap() = (assignment.clone(), Ok(()));
                    }
                    Some((assignment, Ok(()))) => {
                        *declared_name.unwrap() = (assignment.clone(), Err(PathAnalysisErr::MultipleAssignment));
                    }
                    Some((_, Err(PathAnalysisErr::MultipleAssignment))) => {}
                    None => {
                        //FIXME: if the program compiles succesfully, this should never be the case
                        //(assignment to a variable that was not declared beforehand).
                        unreachable!("assignment to a signal/var that has not been declared! this should have not been allowed by the compiler")
                    }
                }
            }
            _ => {},
        }
    }
    assignments
}

/// Control Flow Graph
pub type CFG<'a> = Graph<Node<'a>, usize>;

/// Builds a CFG of the template `t` and runs a path analysis over all paths
/// leading to `reassignment` to check if any path contains more than
/// one assignment to the signal in question.
pub fn check_double_assignment(program: &ProgramArchive, template_name: &str, reassignment: &Meta) -> bool {
    let graph = build_flowgraph(&program, template_name);

    let root= node_index(0);
    let reassignment_node = graph.node_indices().find(|&n| graph.node_weight(n).unwrap().stmt.get_meta().elem_id == reassignment.elem_id).unwrap();
    let paths = algo::all_simple_paths::<Vec<_>, _>(&graph, root, reassignment_node, 0, None)
        .collect::<Vec<_>>();

    if let Statement::Substitution{var, ..} = graph.node_weight(reassignment_node).unwrap().stmt {
        let signal_name = var;
        // Any path from the root up to and including the (potential)
        // reassignment node should have exactly one assignment made to
        // the left-hand side of the assignment.
        if paths.iter().find(|path| 1 != count_assignments(&graph, path, &signal_name)).is_some() {
            return false;
        }
    }
    true
}


/// Counts the number of assignment to signal `signal_name`
fn count_assignments(g: &CFG, path: &Vec<NodeIndex>, signal_name: &str) -> usize {
    path.iter().filter(|&&n| if let Statement::Substitution{var, ..} = g.node_weight(n).unwrap().stmt.clone() { var == signal_name } else { false }).count()
}

/// Builds a Control Flow Graph of the provided template.
pub fn build_flowgraph<'a>(program: &'a ProgramArchive, t: &str) -> CFG<'a> {
    let mut graph = Graph::<Node, usize>::new();
    let parent = vec![];
    _build_flowgraph(program, program.get_template_data(t).get_body(), &mut graph, &parent);
    ////TODO: writing graphviz file for debugging. Remove.
    //use std::fs::File;
    //use std::io::prelude::*;
    //let mut file = File::create("cfg.dot").unwrap();
    //let _ = file.write_all(format!("{}", Dot::new(&graph)).as_bytes());
    graph
}
///
/// Builds a Control Flow Graph of a statement
pub fn build_statement_flowgraph<'a>(program: &'a ProgramArchive, s: &'a Statement) -> CFG<'a> {
    use std::fs::File;
    use std::io::prelude::*;
    let mut graph = Graph::<Node, usize>::new();
    let parent = vec![];
    _build_flowgraph(program, s, &mut graph, &parent);
    let mut file = File::create("loop.dot").unwrap();
    let _ = file.write_all(format!("{}", Dot::new(&graph)).as_bytes());
    graph
}

pub fn all_simple_paths(g: &CFG) -> Vec<Vec<NodeIndex>> {
    let start = node_index(0);
    let end = node_index(g.node_count()-1);
    let paths= algo::all_simple_paths::<Vec<_>, _>(&g, start, end, 0, None)
        .collect::<Vec<_>>();
    paths
}

pub fn assignments_to_x(g: &CFG, path: &[NodeIndex], x: &str) -> Vec<Statement> {
    if g.node_count() == 1 {
        return vec![g.node_weight(node_index(0)).unwrap().stmt.clone()];
    }
    let mut steps = vec![];
    for node in path {
        let stmt = g.node_weight(*node).unwrap().stmt;
        match stmt {
            Statement::Substitution { var, ..} => {
                if var == x {
                    steps.push(stmt.clone());
                }
            }
            _ => {}
        }
    }
    steps
}

fn _build_flowgraph<'a>(program: &'a ProgramArchive,  statement: &'a Statement, g: &mut CFG<'a>, parent: &[NodeIndex]) -> Vec<NodeIndex> {
    use Statement::*;
    match statement {
        Declaration{..}|Substitution{..} => {
            let child= g.add_node(Node{ program, stmt: statement});
            for &p in parent.iter() {
                let _ = g.add_edge(p, child, 0);
            }
            vec![child]
        }
        Block { stmts, .. } => {
            let mut parent= Vec::from(parent);
            for stmt in stmts.iter() {
                parent = _build_flowgraph(program, stmt, g, &parent);
            }
            parent
        }
        InitializationBlock { initializations, .. } => {
            let mut parent= Vec::from(parent);
            for stmt in initializations {
                parent = _build_flowgraph(program, stmt, g, &parent);
            }
            parent
        }
        IfThenElse { if_case, else_case, .. } => {
            let child= g.add_node(Node {program, stmt: statement});
            let mut ret = Vec::from(parent);
            for &p in parent.iter() {
                let _ = g.add_edge(p, child, 0);
            }
            let if_children = _build_flowgraph(program, if_case, g, &mut vec![child]);
            if let Option::Some(else_block) = else_case {
                let else_children = _build_flowgraph(program, else_block, g, &mut vec![child]);
                ret = else_children;
            }
            ret.extend(if_children);
            ret
        }
        While { stmt, .. } => {
            let loop_start= g.add_node(Node { program, stmt: statement});
            for &p in parent.iter() {
                let _ = g.add_edge(p, loop_start, 0);
            }
            let children= _build_flowgraph(program, stmt, g, &[loop_start]);
            let loop_exit= g.add_node(Node { program, stmt});
            for &p in children.iter() {
                let _ = g.add_edge(p, loop_exit, 0);
            }
            let _= g.add_edge(loop_exit, loop_start, 0);
            let _= g.add_edge(loop_start, loop_exit, 0);
            //NOTE: exit_loop node, added to include one cycle of a loop
            vec![loop_exit]
        }
        // None of these statements are relevant to the control flow
        MultSubstitution{..}|Return{..}|UnderscoreSubstitution{..}|ConstraintEquality{..}|LogCall{..}|Assert{..} => {
            let child= g.add_node(Node { program, stmt: statement});
            for &p in parent.iter() {
                let _ = g.add_edge(p, child, 0);
            }
            vec![child]
        }
    }
}


fn print_meta(program: &ProgramArchive, m: &Meta) -> String {
    let sources = program.get_file_library().to_storage();
    let source = sources.get(m.get_file_id()).unwrap();
    let s = source.source().as_str();
    let loc = m.location.clone();
    s[loc].to_string()
}


pub struct Node<'a> {
    stmt: &'a Statement,
    // only needed for dotviz?
    program: &'a ProgramArchive,
}

use std::fmt;
impl<'a> fmt::Display for Node<'a> {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write strictly the first element into the supplied output
        // stream: `f`. Returns `fmt::Result` which indicates whether the
        // operation succeeded or failed. Note that `write!` uses syntax which
        // is very similar to `println!`.
        //
        //
        if let Statement::While { cond, .. } = self.stmt {
            write!(f, "While ({})", print_meta(&self.program, &cond.get_meta()))
        } else if let Statement::IfThenElse{ cond, .. } = self.stmt  {
            write!(f, "If ({})", print_meta(&self.program, &cond.get_meta()))
        } else {
            write!(f, "{}", print_meta(&self.program, &self.stmt.get_meta()))
        }
    }
}



pub struct PathAnalyser<'a> {
    call_flow_graphs: HashMap<String, CFG<'a>>,
    constraint_execution_assignments: HashMap<String, Vec<Meta>>,
    pub checked_for_loops: HashSet<usize>,
}

impl<'a> PathAnalyser<'a> {
    pub fn new(program: &'a ProgramArchive) -> PathAnalyser {

        let call_flow_graphs = program.get_templates().iter().map(|(name, _data)| (name.to_owned(), build_flowgraph(program, name))).collect();

        PathAnalyser { call_flow_graphs, constraint_execution_assignments: HashMap::new(), checked_for_loops: HashSet::new() }
    }

    pub fn constraint_signal_assignment(&mut self, template_name: &str, meta: &Meta, symbol: &str) -> Result<(), PathAnalysisLint> {
        let graph = self.call_flow_graphs.get(template_name).unwrap();

        if let Some(metas) = self.constraint_execution_assignments.get(symbol) {
            for prev_meta in metas {
                let start= graph.node_indices().find(|&n| graph.node_weight(n).unwrap().stmt.get_meta().elem_id == prev_meta.elem_id).unwrap();

                let end= graph.node_indices().find(|&n| graph.node_weight(n).unwrap().stmt.get_meta().elem_id == meta.elem_id).unwrap();

                let paths = algo::all_simple_paths::<Vec<_>, _>(&graph, start, end, 0, None)
                    .collect::<Vec<_>>();

                //
                if paths.len() > 0
                    || start == end { // start == end when the same loop body is iterated more than
                                      // once
                        return Err(PathAnalysisLint{_loc: meta.location.clone(), _meta: meta.clone(), _xtype: VariableType::Signal(SignalType::Output, vec![]), _path_err: PathAnalysisErr::MultipleAssignment, name: symbol.to_owned(), msg: format!("Signal {} is assigned to multiple times", symbol.to_owned())})
                }
            }
        }
        if let Some(metas) = self.constraint_execution_assignments.get_mut(symbol) {
            metas.push(meta.clone());
        } else {
            self.constraint_execution_assignments.insert(symbol.to_owned(), vec![meta.clone()]);
        }
        Ok(())
    }
}
