pub type TabId = u64;
pub type RequestId = u64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BrowserInput {
    pub enter_pressed: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum ResourceKind {
    Html,
    Css,
    Image,
}
