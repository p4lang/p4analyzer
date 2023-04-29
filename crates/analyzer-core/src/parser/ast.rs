use std::rc::Rc;


#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct P4Program {
	pub top_level_declarations: Vec<TopLevelDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TopLevelDeclaration {
	pub annotations: Vec<Annotation>,
	pub kind: TopLevelDeclarationKind,
	/// The length of this node in the source token stream, in tokens.
	pub length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Annotation {
	Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TopLevelDeclarationKind {
	Parser(ParserDeclaration),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParserDeclaration {
	pub parameters: ParameterList,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParameterList {
	pub list: Vec<Parameter>
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Parameter {
	pub annotations: Vec<Annotation>,
	pub direction: Option<Direction>,
	pub typ: Type,
	pub name: Identifier,
	pub length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction {
	In,
	Out,
	InOut,
	Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Identifier {
	pub name: Rc<String>,
	pub length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Type {
	pub name: Identifier,
	pub params: Option<TypeParameters>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeParameters {
	pub list: Vec<TypeParameter>,
	pub length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeParameter {
	pub annotations: Vec<Annotation>,
	pub typ: Type,
	pub length: usize,
}
