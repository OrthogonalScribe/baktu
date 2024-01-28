fn main() -> Result<(), Box<dyn std::error::Error>> {
	use clap::Parser;
	baktu::Baktu::parse().exec()
}
