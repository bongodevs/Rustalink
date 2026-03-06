use crate::audio::buffer::pool::get_byte_pool;

pub struct RingBuffer {
    buf: Vec<u8>,
    size: usize,
    write_offset: usize,
    read_offset: usize,
    length: usize,
}

impl RingBuffer {
    pub fn new(size: usize) -> Self {
        let pool = get_byte_pool();
        let mut buf = pool.acquire(size);
        buf.resize(size, 0);
        Self {
            buf,
            size,
            write_offset: 0,
            read_offset: 0,
            length: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub fn remaining(&self) -> usize {
        self.size - self.length
    }

    /// Write `chunk` into the buffer.  
    /// If the buffer is full, the **oldest** data is overwritten.
    pub fn write(&mut self, chunk: &[u8]) {
        let to_write = chunk.len();
        let available_at_end = self.size - self.write_offset;

        if to_write <= available_at_end {
            self.buf[self.write_offset..self.write_offset + to_write].copy_from_slice(chunk);
        } else {
            self.buf[self.write_offset..].copy_from_slice(&chunk[..available_at_end]);
            self.buf[..to_write - available_at_end].copy_from_slice(&chunk[available_at_end..]);
        }

        let new_len = self.length + to_write;
        if new_len > self.size {
            let overwritten = new_len - self.size;
            self.read_offset = (self.read_offset + overwritten) % self.size;
            self.length = self.size;
        } else {
            self.length = new_len;
        }

        self.write_offset = (self.write_offset + to_write) % self.size;
    }

    /// Read up to `n` bytes, returning them in a pooled `Vec<u8>`.
    pub fn read(&mut self, n: usize) -> Option<Vec<u8>> {
        let to_read = self.peek(n)?;
        self.read_offset = (self.read_offset + to_read.len()) % self.size;
        self.length -= to_read.len();
        Some(to_read)
    }

    /// Peek at up to `n` bytes without advancing the read pointer.
    pub fn peek(&self, n: usize) -> Option<Vec<u8>> {
        let to_read = n.min(self.length);
        if to_read == 0 {
            return None;
        }

        let pool = get_byte_pool();
        let mut out = pool.acquire(to_read);
        out.resize(to_read, 0);

        self.copy_to(&mut out);
        Some(out)
    }

    fn copy_to(&self, out: &mut [u8]) {
        let to_copy = out.len();
        let available_at_end = self.size - self.read_offset;

        if to_copy <= available_at_end {
            out.copy_from_slice(&self.buf[self.read_offset..self.read_offset + to_copy]);
        } else {
            out[..available_at_end].copy_from_slice(&self.buf[self.read_offset..]);
            out[available_at_end..].copy_from_slice(&self.buf[..to_copy - available_at_end]);
        }
    }

    pub fn skip(&mut self, n: usize) -> usize {
        let to_skip = n.min(self.length);
        self.read_offset = (self.read_offset + to_skip) % self.size;
        self.length -= to_skip;
        to_skip
    }

    pub fn clear(&mut self) {
        self.write_offset = 0;
        self.read_offset = 0;
        self.length = 0;
    }

    pub fn dispose(mut self) {
        let pool = get_byte_pool();
        let buf = std::mem::take(&mut self.buf);
        pool.release(buf);
    }
}

impl Drop for RingBuffer {
    fn drop(&mut self) {
        if !self.buf.is_empty() {
            let pool = get_byte_pool();
            let buf = std::mem::take(&mut self.buf);
            pool.release(buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_basic() {
        let mut rb = RingBuffer::new(10);
        assert_eq!(rb.remaining(), 10);

        rb.write(b"hello");
        assert_eq!(rb.len(), 5);
        assert_eq!(rb.remaining(), 5);

        let data = rb.read(3).unwrap();
        assert_eq!(data, b"hel");
        assert_eq!(rb.len(), 2);

        let data = rb.peek(2).unwrap();
        assert_eq!(data, b"lo");
        assert_eq!(rb.len(), 2);

        let data = rb.read(5).unwrap();
        assert_eq!(data, b"lo");
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn test_ring_buffer_wrap_around() {
        let mut rb = RingBuffer::new(10);
        rb.write(b"0123456789");
        rb.skip(5);
        rb.write(b"abcde"); // Wraps around

        let data = rb.read(10).unwrap();
        assert_eq!(data, b"56789abcde");
    }

    #[test]
    fn test_ring_buffer_overwrite() {
        let mut rb = RingBuffer::new(5);
        rb.write(b"12345");
        rb.write(b"67"); // Overwrites "12"

        let data = rb.read(5).unwrap();
        assert_eq!(data, b"34567");
    }

    #[test]
    fn test_ring_buffer_large_write() {
        let mut rb = RingBuffer::new(5);
        rb.write(b"12345678"); // Writes more than capacity

        let data = rb.read(5).unwrap();
        assert_eq!(data, b"45678");
    }
}
