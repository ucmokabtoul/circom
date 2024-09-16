use std::collections::HashMap;
use program_structure::ast::{Statement, VariableType, Expression, Access, Meta, AssignOp, SignalType};
use program_structure::error_code::ReportCode;
use program_structure::program_archive::ProgramArchive;
use program_structure::error_definition::{Report, ReportCollection};
use program_structure::file_definition::{self, FileLocation};
use program_structure::template_data::TemplateData;

pub mod visit {
//! AstVisitor provides a visitor abstraction over an AST.
//!
//! Provides an interface to operate over the syntactic clauses of a Circom
//! AST that are relevant for linting.
    use super::*;
    /// AstVisitor abstraction over an AST.
    pub trait AstVisitor {
        /// Types that need any initializations prior to traversing the AST
        /// should do it here.
        fn init(&mut self, program: &mut ProgramArchive);
        /// Operates on a Statement::Declaration.
        fn visit_declaration(
            &mut self,
            program: &ProgramArchive,
            meta: &Meta,
            xtype: &VariableType,
            name: &String,
            dimensions: &Vec<Expression>,
            is_constant: bool,
        );
        /// Operates on a Statement::Substitution.
        fn visit_substitution(
            &mut self,
            program: &ProgramArchive,
            meta: &Meta,
            var: &str,
            access: &Vec<Access>,
            op: &AssignOp,
            rhe: &Expression,
        );
        fn visit_block(
            &mut self,
            program: &ProgramArchive,
            meta: &Meta,
            stmts: &Vec<Statement>,
        );
        /// Linters should call this method at the end of the AST traversal
        fn lint(&mut self, program: &ProgramArchive) -> Vec<Lint>;
    }
}

use visit::*;

// ComponentInOut tracks assignments to input and output signals to every
// component as per the following structure:
//{
//  "a": (
//    {
//      "in1": "in[0]",
//      "in2": "in[1]"
//    },
//    {
//      "out2": "salida"
//    }
//  )
//}
//


//
type ComponentInOut = HashMap<String, (HashMap<String, String>, HashMap<String, String>)>;

// Tracks literal expressions assigned to signals of all kinds.
type Signals = HashMap<String, Expression>;

struct AnonComponentLint {
    loc: FileLocation,
    template_name: String,
    var: String,
}

struct ConstantSignalLint {
    loc: FileLocation,
    name: String,
}

#[derive(PartialEq, Debug)]
pub enum LintLevel {
    Error,
    Warning,
    Note,
}

#[derive(PartialEq, Debug)]
pub struct Lint {
    pub error_code: ReportCode,
    pub error_msg: String,
    pub loc: FileLocation,
    pub msg: String,
    pub level: LintLevel,
}


/// Implements the ConstantSignalLint
pub struct ConstantSignalLinter {
    constant_signal_lints: Vec<ConstantSignalLint>,
    signals: Vec<String>,
    signals_rhe: Signals,
}

impl ConstantSignalLinter {
    pub fn new() -> ConstantSignalLinter {
        let constant_signal_lints = Vec::<ConstantSignalLint>::new();

        let signals = Vec::new();

        let signals_rhe = Signals::new();

        ConstantSignalLinter { constant_signal_lints, signals, signals_rhe }
    }
}

impl AstVisitor for ConstantSignalLinter {
    /// initializes the is_constant attribute of a Declaration by running
    /// `_handle_template_constants`
    fn init(&mut self, program: &mut ProgramArchive) {
        use crate::decorators::constants_handler::_handle_template_constants;
        for (_, data) in program.get_mut_templates() {
            _handle_template_constants(data);
        }
    }
    fn visit_declaration(
        &mut self,
        _program: &ProgramArchive,
        meta: &Meta,
        xtype: &VariableType,
        name: &String,
        _dimensions: &Vec<Expression>,
        is_constant: bool,
    ) {
        match xtype {
            VariableType::Signal(SignalType::Intermediate, _)
            | VariableType::Signal(SignalType::Output, _) => {
                self.signals.push(name.to_owned());
                if is_constant {
                    self.constant_signal_lints.push(ConstantSignalLint {
                        loc: file_definition::generate_file_location(
                            meta.get_start(),
                            meta.get_end(),
                        ),
                        name: name.to_owned(),
                    });
                }
            }
            _ => {}
        }
    }
    fn visit_substitution(
        &mut self,
        _program: &ProgramArchive,
        _meta: &Meta,
        var: &str,
        _access: &Vec<Access>,
        _op: &AssignOp,
        rhe: &Expression,
    ) {
        if self.signals.contains(&var.to_owned()) {
            self.signals_rhe.insert(var.to_owned(), rhe.clone());
        }
    }
    fn visit_block(
        &mut self,
        _program: &ProgramArchive,
        _meta: &Meta,
        _stmts: &Vec<Statement>,
    ) {
    }
    fn lint(&mut self, program: &ProgramArchive) -> Vec<Lint> {
        let mut lints = Vec::new();
        for lint in &self.constant_signal_lints {
            let name = &lint.name;
            let rhe = self.signals_rhe.get(name).unwrap();
            lints.push(Lint {
                error_code: ReportCode::ConstantSignalLint,
                error_msg: format!("Constant signal: `{}`", name),
                loc: lint.loc.clone(),
                msg: format!(
                    "You should define `{}` as a variable instead: `var {} = {}`",
                    name,
                    name,
                    print_expr(program, rhe)
                ),
                level: LintLevel::Note,
            });
        }
        lints
    }
}

/// Implements the AnonComponentLint
pub struct AnonComponentLinter {
    components: ComponentInOut,
    anon_component_lints: Vec<AnonComponentLint>,
}

impl AstVisitor for AnonComponentLinter {
    fn init(&mut self, _program: &mut ProgramArchive) {}
    // Keeps track of component declarations that appear in an AST in order
    // to provide the Anonymous Component lint.
    fn visit_declaration(
        &mut self,
        program: &ProgramArchive,
        meta: &Meta,
        xtype: &VariableType,
        name: &String,
        dimensions: &Vec<Expression>,
        _is_constant: bool,
    ) {
        if let VariableType::Component = xtype {
            self.components.insert(name.to_owned(), (HashMap::new(), HashMap::new()));
            let template_name = meta.clone().component_inference.unwrap();
            for e in dimensions {
                print_expr(program, e);
            }
            self.anon_component_lints.push(AnonComponentLint {
                loc: file_definition::generate_file_location(meta.get_start(), meta.get_end()),
                template_name,
                var: name.to_owned(),
            });
        }
    }

    // Parses a Substitution in order to store the different initializers of a
    // given component.
    fn visit_substitution(
        &mut self,
        program: &ProgramArchive,
        _meta: &Meta,
        var: &str,
        access: &Vec<Access>,
        _op: &AssignOp,
        rhe: &Expression,
    ) {
        let lhe_access = access;
        match rhe {
            Expression::Variable { name, access, .. } => {
                for a in lhe_access {
                    match a {
                        Access::ComponentAccess(sub_component) => {
                            let rhe_expr = print_expr(program, rhe);
                            self.components
                                .get_mut(var)
                                .unwrap()
                                .0
                                .insert(sub_component.to_owned(), rhe_expr);
                        }
                        Access::ArrayAccess(_sub_component) => {
                            //FIXME: implement?
                        }
                    }
                }
                for a in access {
                    match a {
                        Access::ComponentAccess(sub_component) => {
                            self.components
                                .get_mut(name)
                                .unwrap()
                                .1
                                .insert(sub_component.to_owned(), var.to_owned());
                        }
                        Access::ArrayAccess(_sub_component) => {
                            //FIXME: implement?
                        }
                    }
                }
            }
            _ => {}
        }
    }
    fn visit_block(
        &mut self,
        _program: &ProgramArchive,
        _meta: &Meta,
        _stmts: &Vec<Statement>,
    ) {
    }
    fn lint(&mut self, program: &ProgramArchive) -> Vec<Lint> {
        let mut lints = Vec::new();
        for lint in &self.anon_component_lints {
            let template_name = lint.template_name.clone();
            let data = program.get_template_data(&template_name);
            let name = &lint.var;
            lints.push(Lint {
                error_code: ReportCode::AnonymousCompLint,
                error_msg: format!("Anonymous component: `{}`", name),
                loc: lint.loc.clone(),
                msg: format!(
                    "You can use ({}) <== {template_name}()({});",
                    fmt_component_outs(data, &self.components.get(name).unwrap().1),
                    fmt_component_args(data, &self.components.get(name).unwrap().0)
                ),
                level: LintLevel::Note,
            });
        }
        lints
    }
}

impl AnonComponentLinter {
    pub fn new() -> AnonComponentLinter {
        // A hashmap to track declared components and store references to their
        // inputs/outputs.
        let components = HashMap::new();
        let anon_component_lints = Vec::<AnonComponentLint>::new();

        AnonComponentLinter { components, anon_component_lints }
    }
}

/// Provides an API to run a set of registered lints over a program
pub struct StaticLinter {
    program: ProgramArchive,
    pub linters: Vec<Box<dyn AstVisitor>>,
}

impl StaticLinter {
    /// Creates a linter with the provided linter commands.
    pub fn new(program: ProgramArchive, linters: Vec<Box<dyn AstVisitor>>) -> StaticLinter {
        //let mut template_data = template_data.clone();
        //_handle_template_constants(&mut template_data);
        //let reports = ReportCollection::new();
        //// A hashmap to track declared components and store references to their
        //// inputs/outputs.
        //let components = HashMap::new();
        //let anon_component_lints = Vec::<AnonComponentLint>::new();
        //let constant_signal_lints = Vec::<ConstantSignalLint>::new();
        //
        //let signals = Vec::new();
        //
        //let signals_rhe = Signals::new();

        StaticLinter { program, linters }
    }
    /// Callers should only call this method to run their linters on a given
    /// program.
    /// The lints are returned in the order of declaration of the linters.
    pub fn lint(&mut self) -> Vec<Lint> {
        let mut lints = Vec::new();
        for l in self.linters.iter_mut() {
            l.init(&mut self.program);
        }
        let program = self.program.clone();
        let templates = program.get_templates().values();
        for template_data in templates {
            self.walk_ast(template_data.get_body());
        }
        for l in self.linters.iter_mut() {
            lints.extend(l.lint(&self.program));
        }
        lints
    }
    pub fn walk_ast(&mut self, s: &Statement) {
        use Statement::*;
        match s {
            IfThenElse { if_case, else_case, .. } => {
                self.walk_ast(if_case);
                if let Option::Some(else_block) = else_case {
                    self.walk_ast(else_block);
                }
            }
            While { stmt, .. } => {
                self.walk_ast(stmt);
            }
            Block { meta, stmts, .. } => {
                for l in self.linters.iter_mut() {
                    l.visit_block(&self.program, meta, stmts);
                }
                for stmt in stmts.iter() {
                    self.walk_ast(stmt);
                }
            }
            InitializationBlock { initializations, .. } => {
                for stmt in initializations {
                    self.walk_ast(stmt);
                }
            }
            Declaration { meta, xtype, name, dimensions, is_constant } => {
                for l in self.linters.iter_mut() {
                    l.visit_declaration(&self.program, meta, xtype, name, dimensions, *is_constant);
                }
            }
            Substitution { meta, var, rhe, access, op } => {
                for l in self.linters.iter_mut() {
                    l.visit_substitution(&self.program, meta, var, access, op, rhe);
                }
            }
            //TODO: are there more syntactic clauses that are relevant for this analysis?
            _ => {}
        }
    }

}

pub fn report_lints(program: &ProgramArchive) -> Option<ReportCollection> {
    let file_id = program.get_file_id_main().clone();
    let mut analyser = StaticLinter::new(program.clone(), vec![Box::new(AnonComponentLinter::new()), Box::new(ConstantSignalLinter::new())]);
    let mut reports = ReportCollection::new();
    let lints = analyser.lint();
    for lint in lints {
        let mut report = match lint.error_code {
            ReportCode::AnonymousCompLint => Report::note(lint.error_msg, lint.error_code),
            ReportCode::ConstantSignalLint => Report::note(lint.error_msg, lint.error_code),
            ReportCode::LoopNoProgress => Report::error(lint.error_msg, lint.error_code),
            ReportCode::LoopMayOverflow => Report::error(lint.error_msg, lint.error_code),
            _ => unreachable!(""),
        };
        report.add_primary(lint.loc, file_id, lint.msg);
        reports.push(report);
    }
    if reports.is_empty() {
        None
    } else {
        Some(reports)
    }
}

/// print_expr returns the expression e as it literally appeared in program.
fn print_expr(program: &ProgramArchive, e: &Expression) -> String {
    let sources = program.get_file_library().to_storage();
    let source = sources.get(e.get_meta().get_file_id()).unwrap();
    let s = source.source().as_str();
    let loc = e.get_meta().location.clone();
    s[loc].to_string()
}

/// fmt_component_args formats the arguments to a component.
fn fmt_component_args(data: &TemplateData, inputs: &HashMap<String, String>) -> String {
    //FIXME: unwrap()
    data.get_declaration_inputs()
        .iter()
        .map(|v| inputs.get(&v.0).unwrap().to_owned())
        .collect::<Vec<String>>()
        .join(", ")
}
/// fmt_component_args formats the left-hand side values of an assignment from a component.
fn fmt_component_outs(data: &TemplateData, outputs: &HashMap<String, String>) -> String {
    data.get_declaration_outputs()
        .iter()
        .map(|v| if let Some(vv) = outputs.get(&v.0) { vv.to_owned() } else { "_".to_owned() })
        .collect::<Vec<String>>()
        .join(", ")
}

