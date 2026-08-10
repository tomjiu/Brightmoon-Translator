use llama_cpp::LlamaModel;

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("usage: llama-spike <model.gguf>");
    println!("loading: {path}");
    let model = LlamaModel::load_from_file(&path).expect("Could not load model");
    println!("loaded OK");
    let prompt = "Translate the following segment into Chinese, without additional explanation. Hello";
    let mut ctx = model.create_session();
    ctx.advance_context(prompt.as_bytes()).expect("advance context");
    let mut out = String::new();
    let mut completions = ctx.start_completing();
    while let Some(next_token) = completions.next_token() {
        out.push_str(&String::from_utf8_lossy(&*next_token.detokenize()));
    }
    println!("output: {out}");
    Ok(())
}
