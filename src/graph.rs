use stream;

use typed_arena;

struct Graph {
    arena: typed_arena::Arena<stream::Token>,
}

impl Graph {
    pub fn from_stream(parser: stream::TokenParser) -> Result<Graph, String> {
        token_use!();

        unimplemented!()
    }

    pub fn from_string(pat: &str) -> Result<Graph, String> {
        let tokens = stream::TokenParser::from_string(pat)?;
        Graph::from_stream(tokens)
    }
}
