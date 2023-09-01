pub trait CeilingDiv {
	fn ceiling_div(self, other: Self) -> Self;
}

impl CeilingDiv for u32 {
	fn ceiling_div(self, other: Self) -> Self {
		(self + other - 1) / other
	}
}
