pub(crate) struct ByteCursor<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl<'a> ByteCursor<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, index: 0 }
    }

    pub(crate) fn next(&mut self) -> u8 {
        if self.bytes.is_empty() {
            return 0;
        }

        let value = self.bytes[self.index % self.bytes.len()];
        self.index = self.index.saturating_add(1);
        value
    }

    pub(crate) fn choose_index(&mut self, len: usize) -> usize {
        if len == 0 {
            0
        } else {
            usize::from(self.next()) % len
        }
    }

    pub(crate) fn choose_str<'b>(&mut self, values: &'b [&'b str]) -> &'b str {
        values[self.choose_index(values.len())]
    }

    pub(crate) fn next_usize(&mut self, upper_bound: usize) -> usize {
        if upper_bound == 0 {
            0
        } else {
            usize::from(self.next()) % upper_bound
        }
    }

    pub(crate) fn next_bool(&mut self) -> bool {
        self.next() & 1 == 0
    }
}
