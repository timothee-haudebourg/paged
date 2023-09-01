/// Module.
pub struct Module {
	pages: Pages<Page>
}

/// Dataset.
pub struct Dataset {
	graphs: List<GraphDescription>
}

/// Graph description.
pub struct GraphDescription {
	pages: PageRef<Graph>
}