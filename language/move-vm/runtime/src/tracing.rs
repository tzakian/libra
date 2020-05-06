use vm::file_format::Bytecode;
use std::fs::OpenOptions;

trait Tracer {
    const FILE_PATH: &'static str;
    fn trace(function_path: &str, pc: u64, instruction: &Bytecode);
}

pub struct EmptyTracer;
impl Tracer for EmptyTracer {
    const FILE_PATH: &'static str  = "";
    fn trace(_function_path: &str, _pc: u64, _instruction: &Bytecode) { }
}

pub struct ExecutionTracer;

impl Tracer for ExecutionTracer {
    const FILE_PATH: &'static str = "Testingfile.trace";
    fn trace(function_path: &str, pc: u64, instruction: &Bytecode) {
        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(Self::FILE_PATH)
            .unwrap();
        writeln!(file, "{}\t{}\t{:?}", function_path, pc, instruction)
    }
}
