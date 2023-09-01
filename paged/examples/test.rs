use paged::{HeapSection, Paged, Section};

#[derive(Paged)]
pub struct Header {
	interpretation: Interpretation,
	graphs: Section<Graph>,
	data: HeapSection,
}

#[derive(Paged)]
pub struct Interpretation {
	iris: Section<Iri>,
	literals: Section<Literal>,
	resources: Section<InterpretedResource>,
}

#[derive(Paged)]
pub struct Dataset {
	default_graph: GraphDescription,
	named_graphs: Section<Graph>,
}

#[derive(Paged)]
pub struct Iri {
	value: String,
	id: u32,
}

#[derive(Paged)]
pub struct Literal {
	value: String,
}

#[derive(Paged)]
pub struct InterpretedResource {
	id: u32,
	iris: Vec<u32>,
	literal: Vec<u32>,
	ne: Vec<u32>,
}

#[derive(Paged)]
pub struct Graph {
	id: u32,
	description: GraphDescription,
}

#[derive(Paged)]
pub struct GraphDescription {
	triples: Section<Triple>,
	resources: Section<GraphResource>,
}

#[derive(Paged)]
pub struct GraphResource {
	as_subject: Vec<u32>,
	as_predicate: Vec<u32>,
	as_object: Vec<u32>,
}

#[derive(Paged)]
pub struct Triple {
	subject: u32,
	predicate: u32,
	object: u32,
}

fn main() {
	// ...
}
