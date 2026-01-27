pub const BLOCK_TEMPLATE: &str = "<div class=box><span>hello</span><img src=x></div>";

pub fn make_blocks(blocks: usize) -> String {
    let mut html = String::with_capacity(BLOCK_TEMPLATE.len() * blocks);
    for _ in 0..blocks {
        html.push_str(BLOCK_TEMPLATE);
    }
    html
}
