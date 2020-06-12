use std::io::prelude::*;

pub const DEFAULT_BUF_SIZE: usize = 8196;

use std::io::{self};

use std::cmp;
use std::fmt;

extern crate log;


pub struct BufferedReader<R> {
    inner: R,
    buf: Box<[u8]>,
    pos: usize,
    cap: usize,
    mark: isize,
    ahead: usize
 }

 pub trait MarkRead : Read {
     fn mark(&mut self, read_limit: usize) -> io::Result<()>;
     fn reset(&mut self) -> io::Result<()>;
 }

 impl<R: Read> BufferedReader<R> {

    pub fn new(inner: R) -> BufferedReader<R> {
        BufferedReader::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    pub fn with_capacity(capacity: usize, inner: R) -> BufferedReader<R> {
        let mut buffer = Vec::with_capacity(capacity);
        buffer.resize_with(capacity, Default::default);
        BufferedReader { inner, buf: buffer.into_boxed_slice(), pos: 0, cap: 0, mark: -1, ahead: 0 }        
    }

    
}

impl<R> BufferedReader<R> {
    pub fn buffer(&self) -> &[u8] {
        &self.buf[self.pos..self.cap]
    }    

    

    fn resize_buf(&mut self, new_length: usize) -> io::Result<()> {
        let mut new_buffer = self.buf.to_vec();
        new_buffer.resize_with(new_length, Default::default);
        self.buf = new_buffer.into_boxed_slice();
        Ok(())
    }
}

impl<R: Read> Read for BufferedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {

        // resize the buffer if needed
        if buf.len() > self.buf.len() {
            let _ = self.resize_buf(buf.len());
        }
        
        // fill the buffer if needed
        if (self.cap - self.pos) < buf.len()  {
            // we need to fill the buffer
            let _ = self.fill_buf();
        }

        // there is enough space in the buffer
        // do a simple copy
        let min_length = std::cmp::min(self.cap, buf.len());
        
        // set the indices
        let starting_index = self.pos;
        let ending_index = self.pos + min_length;            

        // set the slices
        let target_slice = &mut buf[0..min_length];
        let source_slice = &self.buf[starting_index..ending_index];

        // perform copy
        target_slice.copy_from_slice(source_slice);

        self.consume(min_length);

        return Ok(min_length);
    }
}

impl<R: Read> MarkRead for BufferedReader<R> {
    fn reset(&mut self) -> io::Result<()> {
        if self.mark >= 0 {
            self.pos = self.mark as usize;
        }

        Ok(())
    }

    fn mark(&mut self, read_limit: usize) -> io::Result<()> {
        // check if the buffer can hold the read_limit
        // if not then allocate
        if read_limit > self.buf.len() {
            let _ = self.resize_buf(read_limit);
        }

        // fill the buffer if needed
        if (self.cap - self.pos) < read_limit  {
            // we need to fill the buffer
            let _ = self.fill_buf();
        }

        self.mark = self.pos as isize;
        self.ahead = read_limit;

        Ok(())
    }
}

impl<R: Read> BufRead for BufferedReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {

        // does the buffer need to be re-filled?
        if self.pos >= self.cap {
            debug_assert!(self.pos == self.cap);
            self.cap = self.inner.read(&mut self.buf)?;
            self.pos = 0;
            self.mark = -1;
        }
        // we need to do a partial read
        else {
            // shift the data from pos to zero
            self.buf.copy_within(self.pos.., 0);
            let nread = self.inner.read(&mut self.buf[self.cap..])?;
            self.cap += nread;
            self.pos = 0;
            self.mark = -1;
        }

        Ok(&self.buf[self.pos..self.cap])
    }

    fn consume(&mut self, amt: usize) {
        self.pos = cmp::min(self.pos + amt, self.cap);
        
        // do we need to invalidate the mark
        if self.mark > -1 {
            if self.pos > self.mark as usize + self.ahead {
                self.mark = -1;
            }
        }
    }
}

impl<R> fmt::Debug for BufferedReader<R>
where
    R: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("BufferedReader")
            .field("reader", &self.inner)
            .field("buffer", &format_args!("{}/{}", self.cap - self.pos, self.buf.len()))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::io::prelude::*;
    //use log::Level;
    //use std::io::{SeekFrom};
    
    use crate::BufferedReader;

    //use BufferedReader;
    use crate::MarkRead;


    use log::{info};

    /// A dummy reader intended at testing short-reads propagation.
    pub struct ShortReader {
        lengths: Vec<usize>,
    }

    impl Read for ShortReader {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            if self.lengths.is_empty() { Ok(0) } else { Ok(self.lengths.remove(0)) }
        }
    }

    #[test]
    fn test_buffered_reader() {
        //env_logger::init();

        let inner: &[u8] = &[5, 6, 7, 0, 1, 2, 3, 4];
        let mut reader = BufferedReader::with_capacity(2, inner);

        let mut buf = [0, 0, 0];
        let nread = reader.read(&mut buf);
        assert_eq!(nread.unwrap(), 3);
        assert_eq!(buf, [5, 6, 7]);
        info!("{:?}", reader.buffer());
        info!("{:?} months in a year.", 12);
        info!("reader [{:?}]", reader);
        assert_eq!(reader.buffer(), []);

        let mut buf = [0, 0];
        let nread = reader.read(&mut buf);
        assert_eq!(nread.unwrap(), 2);
        assert_eq!(buf, [0, 1]);
        assert_eq!(reader.buffer(), [2]);

        let mut buf = [0];
        let nread = reader.read(&mut buf);
        assert_eq!(nread.unwrap(), 1);
        assert_eq!(buf, [2]);
        assert_eq!(reader.buffer(), []);

        let mut buf = [0, 0, 0];
        let nread = reader.read(&mut buf);
        assert_eq!(nread.unwrap(), 2);
        assert_eq!(buf, [3, 4, 0]);
        assert_eq!(reader.buffer(), []);

        assert_eq!(reader.read(&mut buf).unwrap(), 0);
    }
   
    #[test]
    fn test_buffered_mark() {
        //env_logger::init();

        let inner: &[u8] = &[5, 6, 7, 0, 1, 2, 3, 4];
        let mut reader = BufferedReader::with_capacity(2, inner);

        let _ = reader.mark(2);
        let mut buf = [0, 0, 0];
        let nread = reader.read(&mut buf);
        assert_eq!(nread.unwrap(), 3);
        assert_eq!(buf, [5, 6, 7]);
        info!("{:?}", reader.buffer());
        info!("{:?} months in a year.", 12);
        info!("reader [{:?}]", reader);
        assert_eq!(reader.buffer(), []);

        let _ = reader.mark(2);
        let mut buf = [0, 0];
        let nread = reader.read(&mut buf);
        assert_eq!(nread.unwrap(), 2);
        assert_eq!(buf, [0, 1]);
        assert_eq!(reader.buffer(), [2]);

        let mut buf = [0];
        let nread = reader.read(&mut buf);
        assert_eq!(nread.unwrap(), 1);
        assert_eq!(buf, [2]);
        assert_eq!(reader.buffer(), []);

        let mut buf = [0, 0, 0];
        let nread = reader.read(&mut buf);
        assert_eq!(nread.unwrap(), 2);
        assert_eq!(buf, [3, 4, 0]);
        assert_eq!(reader.buffer(), []);

        assert_eq!(reader.read(&mut buf).unwrap(), 0);
    }


    #[test]
    fn test_buffered_reset() {
        //env_logger::init();

        let inner: &[u8] = &[5, 6, 7, 0, 1, 2, 3, 4];
        let mut reader = BufferedReader::with_capacity(2, inner);

        let _ = reader.mark(2);
        let mut buf = [0, 0, 0];
        let nread = reader.read(&mut buf);
        assert_eq!(nread.unwrap(), 3);
        assert_eq!(buf, [5, 6, 7]);
        info!("{:?}", reader.buffer());
        info!("{:?} months in a year.", 12);
        info!("reader [{:?}]", reader);
        assert_eq!(reader.buffer(), []);

        // should do nothing
        let _ = reader.reset();

        let _ = reader.mark(2);
        let mut buf = [0, 0];
        let nread = reader.read(&mut buf);
        assert_eq!(nread.unwrap(), 2);
        assert_eq!(buf, [0, 1]);
        assert_eq!(reader.buffer(), [2]);

        // should work
        let _ = reader.reset();

        // read the buffer again
        buf = [0, 0];
        let nread = reader.read(&mut buf);
        assert_eq!(nread.unwrap(), 2);
        assert_eq!(buf, [0, 1]);
        assert_eq!(reader.buffer(), [2]);


        let mut buf = [0];
        let nread = reader.read(&mut buf);
        assert_eq!(nread.unwrap(), 1);
        assert_eq!(buf, [2]);
        assert_eq!(reader.buffer(), []);

        // should do nothing
        let _ = reader.reset();

        let mut buf = [0, 0, 0];
        let nread = reader.read(&mut buf);
        assert_eq!(nread.unwrap(), 2);
        assert_eq!(buf, [3, 4, 0]);
        assert_eq!(reader.buffer(), []);

        assert_eq!(reader.read(&mut buf).unwrap(), 0);
    }
}